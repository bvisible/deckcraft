// //// Neoffice — added file (no upstream equivalent): the connection of the Nora pane in the browser (WordCraft's, for DeckCraft).

//! The Nora pane's connection for the browser build: a `fetch` made with the page's own session.
//!
//! Nora lives on the Neoffice server, so the editor needs the session of whoever hosts it. The page that hosts the
//! editor turns the connection on in its URL: `?nora` (calls go to the page's own origin, as for an editor served by
//! the instance itself). The pane then opens by itself, as the browser build keeps no setting between visits. The
//! address names another origin (`?nora=<origin>`) only for the workshop's stand-in server, loopback to loopback;
//! any other origin it names is refused (`crate::nora_origin` has the rules), so a link can't send the person's
//! conversation and presentation elsewhere.
//!
//! The host can also describe the connection in `window.deckcraftNora`, read at every call:
//! `{ origin?: string, headers?: { [name]: value }, credentials?: "include" | "same-origin" | "omit" }`; its `origin`
//! may be any. A Frappe page's CSRF token (`window.csrf_token` or `frappe.csrf_token`) is sent as
//! `X-Frappe-CSRF-Token` without being asked; it and the host's `headers` go only with a call to the page's own
//! origin. Nothing secret travels in the URL.
//!
//! A call that gets no answer from the network comes back as a failure the pane can show.
//!
//! A host that makes the calls itself offers `window.deckcraftNoraCall(path, body)`, on the page or on the page of the
//! same origin that frames it, a promise of `{ status, body }` (`body` is the JSON of the call): the pane is then on
//! without `?nora`, and every call goes through it. Neodesk does so: it calls with the person's credential, which
//! neither its page nor the editor ever holds (the same as WordCraft's `wordcraftNoraCall`).

use std::sync::{Arc, Mutex};

use deckcraft_ui_egui::{HttpRequest, HttpResponse, NoraInbox, Services};
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};

/// The origin Nora is asked at when the page's address asks for her (its `nora` parameter, read by name with
/// `URLSearchParams`): the page's own origin, or the loopback one the address names from a loopback page. `None`
/// when the address doesn't ask. Another origin the address names is refused, and the page's own is used.
pub fn requested() -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let param = web_sys::UrlSearchParams::new_with_str(&search).ok()?.get("nora")?;
    let page = window.location().origin().unwrap_or_default();
    Some(crate::nora_origin::from_param(&param, &page).unwrap_or_else(|why| {
        log::warn!("{why}");
        crate::nora_origin::from_param("", &page).unwrap_or_default()
    }))
}

/// Whether the page, or the page of the same origin that frames it, makes Nora's calls itself (`deckcraftNoraCall`):
/// the pane is then on, as with `?nora`.
pub fn offered() -> bool {
    host_call().is_some()
}

/// The host's own way to Nora, when it offers one: the window that offers it, and the function.
fn host_call() -> Option<(JsValue, js_sys::Function)> {
    let window = web_sys::window()?;
    let own = JsValue::from(window.clone());
    let mut windows = vec![own.clone()];
    if let Ok(Some(parent)) = window.parent() {
        let parent = JsValue::from(parent);
        // A top page is its own parent; another origin's `document` can't be read.
        if parent != own && get(&parent, "document").is_some() {
            windows.push(parent);
        }
    }
    windows.into_iter().find_map(|w| get(&w, "deckcraftNoraCall").filter(JsValue::is_function).map(|f| (w, js_sys::Function::from(f))))
}

/// One call made by the host: its answer read as a `fetch`'s (status 0 when it gave none).
async fn call_host(owner: &JsValue, function: &js_sys::Function, request: &HttpRequest) -> Result<(u16, String), String> {
    let promise = function.call2(owner, &JsValue::from_str(&request.path), &JsValue::from_str(&request.body.to_string())).map_err(reason)?;
    let answer = JsFuture::from(js_sys::Promise::resolve(&promise)).await.map_err(reason)?;
    let status =
        get(&answer, "status").and_then(|value| value.as_f64()).filter(|status| (0.0..1000.0).contains(status)).map_or(0, |status| status as u16);
    let body = get(&answer, "body").and_then(|value| value.as_string()).unwrap_or_default();
    Ok((status, body))
}

/// Gives the app the means to reach Nora: each call is a `fetch` (or the host's own call), each answer is delivered to
/// the pane and wakes the interface.
pub fn connect(services: &mut Services, origin: String, ctx: egui::Context) {
    let inbox: NoraInbox = Arc::new(Mutex::new(Vec::new()));
    services.nora_inbox = Some(inbox.clone());
    services.nora_http = Some(Box::new(move |request: HttpRequest| {
        let (inbox, ctx, origin) = (inbox.clone(), ctx.clone(), origin.clone());
        spawn_local(async move {
            let response = call(&origin, &request).await;
            inbox.lock().unwrap_or_else(|e| e.into_inner()).push(response);
            ctx.request_repaint();
        });
    }));
}

/// One call, answered whatever happens: a network failure becomes a refusal with its reason, which the session shows
/// like any other.
async fn call(default_origin: &str, request: &HttpRequest) -> HttpResponse {
    match fetch(default_origin, request).await {
        Ok((status, body)) => HttpResponse { id: request.id, status, body },
        Err(error) => {
            let body = serde_json::json!({ "exception": format!("No answer from Nora: {error}") }).to_string();
            HttpResponse { id: request.id, status: 0, body }
        }
    }
}

/// What the host said in `window.deckcraftNora`.
struct Host {
    origin: Option<String>,
    headers: Vec<(String, String)>,
    credentials: Option<web_sys::RequestCredentials>,
}

fn get(target: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(target, &JsValue::from_str(key)).ok().filter(|value| !value.is_undefined() && !value.is_null())
}

fn host(window: &web_sys::Window) -> Host {
    let mut host = Host { origin: None, headers: Vec::new(), credentials: None };
    if let Some(settings) = get(window, "deckcraftNora").filter(JsValue::is_object) {
        host.origin = get(&settings, "origin").and_then(|value| value.as_string());
        host.credentials = get(&settings, "credentials").and_then(|value| value.as_string()).and_then(|name| match name.as_str() {
            "include" => Some(web_sys::RequestCredentials::Include),
            "same-origin" => Some(web_sys::RequestCredentials::SameOrigin),
            "omit" => Some(web_sys::RequestCredentials::Omit),
            _ => None,
        });
        if let Some(headers) = get(&settings, "headers").filter(JsValue::is_object) {
            for entry in js_sys::Object::entries(&js_sys::Object::from(headers)).iter() {
                let pair = js_sys::Array::from(&entry);
                if let (Some(name), Some(value)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
                    host.headers.push((name, value));
                }
            }
        }
    }
    host
}

/// The CSRF token of a Frappe page, if the page has one.
fn csrf_token(window: &web_sys::Window) -> Option<String> {
    get(window, "csrf_token")
        .and_then(|token| token.as_string())
        .or_else(|| get(window, "frappe").and_then(|frappe| get(&frappe, "csrf_token")).and_then(|token| token.as_string()))
        .filter(|token| !token.is_empty())
}

fn reason(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_sys::Reflect::get(&error, &JsValue::from_str("message")).ok().and_then(|message| message.as_string()))
        .unwrap_or_else(|| format!("{error:?}"))
}

async fn fetch(default_origin: &str, request: &HttpRequest) -> Result<(u16, String), String> {
    if let Some((owner, function)) = host_call() {
        return call_host(&owner, &function, request).await;
    }
    let window = web_sys::window().ok_or("no window")?;
    let host = host(&window);
    let origin = host.origin.as_deref().unwrap_or(default_origin).trim_end_matches('/');
    let url = format!("{origin}{}", request.path);
    let headers = web_sys::Headers::new().map_err(reason)?;
    headers.set("Content-Type", "application/json").map_err(reason)?;
    headers.set("Accept", "application/json").map_err(reason)?;
    // The page's credentials (its CSRF token, the host's headers) go only to the page's own origin.
    let page = window.location().origin().unwrap_or_default();
    if origin.is_empty() || crate::nora_origin::is_page_origin(origin, &page) {
        if let Some(token) = csrf_token(&window) {
            headers.set("X-Frappe-CSRF-Token", &token).map_err(reason)?;
        }
        for (name, value) in &host.headers {
            headers.set(name, value).map_err(reason)?;
        }
    }
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(&request.body.to_string()));
    if let Some(credentials) = host.credentials {
        init.set_credentials(credentials);
    }
    let answer = JsFuture::from(window.fetch_with_str_and_init(&url, &init)).await.map_err(reason)?;
    let response: web_sys::Response = answer.dyn_into().map_err(|_| "the answer is not a response".to_string())?;
    let text = JsFuture::from(response.text().map_err(reason)?).await.map_err(reason)?;
    Ok((response.status(), text.as_string().unwrap_or_default()))
}
