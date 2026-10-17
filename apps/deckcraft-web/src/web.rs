//! The browser shell: web `Services`, drag-and-drop, and the eframe web runner.

use deckcraft_engine::Session;
use deckcraft_ui_egui::{Inbox, Services, SlideApp};
use wasm_bindgen::JsCast as _;
// //// Neoffice — the bridge to the page that frames the editor (`?host=parent`).
use crate::host;
// //// Neoffice — added module: the Nora pane's connection (a `fetch` with the page's own session, or the host's own call).
mod nora;

const PRESENTATION_EXTS: &[&str] = &["deckcraft", "pptx", "potx", "ppsx", "txt", "md"];
const PICTURE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];
const AUDIO_EXTS: &[&str] = &["wav", "mp3", "m4a", "aac", "flac", "ogg", "opus", "aif", "aiff"];
const VIDEO_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm"];
const CANVAS_ID: &str = "deckcraft_canvas";
const LOADING_ID: &str = "deckcraft_loading";

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let mut options = eframe::WebOptions::default();
        // //// Neoffice — the address's flags are read by name (`has_param`), not found anywhere in its text.
        if has_param("webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        // //// Neoffice — a page that frames the editor and asks for the bridge (`?host=parent`) sends it the
        // presentation to edit and takes what it saves.
        let bridge = host::Bridge::requested();
        let app_bridge = bridge.clone();
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    if let Some(rs) = &cc.wgpu_render_state {
                        log::info!("deckcraft-web {}: wgpu backend {:?}", env!("CARGO_PKG_VERSION"), rs.adapter.get_info().backend);
                    }
                    let inbox: Inbox = Inbox::default();
                    let mut services = services(inbox.clone(), cc.egui_ctx.clone());
                    // //// Neoffice — the hosting page gets the saves and the exports, and sends the files.
                    if let Some(bridge) = &app_bridge {
                        bridge.connect(&mut services, inbox.clone(), cc.egui_ctx.clone());
                    }
                    // //// Neoffice — a page that asks for Nora (`?nora`) gets the pane, connected with the page's own
                    // session (another origin only as `nora::requested` allows). The browser build keeps no setting, so the
                    // host opens it. A host that makes the calls itself (`deckcraftNoraCall`, Neodesk) turns it on the same way.
                    let nora = nora::requested().or_else(|| nora::offered().then(String::new));
                    if let Some(origin) = &nora {
                        nora::connect(&mut services, origin.clone(), cc.egui_ctx.clone());
                    }
                    let mut app = SlideApp::new(Session::new(), services);
                    // //// Neoffice — a hosting page sends the presentation to edit: until it does the editor
                    // starts on a blank one (what `?blank` opens), not on the sample deck. The flags are read by name.
                    if !has_param("blank") && app_bridge.is_none() {
                        if let Err(e) = deckcraft_engine::sample::open_sample(&mut app.session) {
                            log::error!("sample deck: {e}");
                        }
                    } else if let Err(e) = app.run("file.new", serde_json::json!({})) {
                        log::error!("new presentation: {e}");
                    }
                    // //// Neoffice — from here on the page that frames the editor opens the presentations
                    // (`Services::host_owns_documents`); the blank one above is the editor's own start.
                    if app_bridge.is_some() {
                        app.services.host_owns_documents = true;
                    }
                    // //// Neoffice — the hosting page learns the editor listens, and names the user who signs the comments.
                    if let Some(bridge) = &app_bridge {
                        if let Some(author) = host::Bridge::author() {
                            app.session.prefs.author = author;
                        }
                        bridge.ready(&app);
                    }
                    // //// Neoffice — see `nora` above: the pane opens by itself when the page offers Nora.
                    if nora.is_some()
                        && let Err(e) = app.run("ui.nora", serde_json::json!({"on": true}))
                    {
                        log::error!("the Nora pane could not be opened: {e}");
                    }
                    // //// Neoffice — by name: `show` in a host name (`?nora=https://showroom…`) or in a name the page
                    // passes (`?author=`) started the slide show over the editor and its Nora pane.
                    if has_param("show") {
                        app.start_show(0, false);
                    }
                    // //// Neoffice — the shell keeps the bridge to the hosting page.
                    Ok(Box::new(WebShell { app, inbox, bridge: app_bridge }))
                }),
            )
            .await;
        // //// Neoffice — a hosting page hears that the editor could not start (no WebGPU nor WebGL2, for example).
        if let (Err(e), Some(bridge)) = (&result, &bridge) {
            bridge.post_failed(&format!("{e:?}"));
        }
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>DeckCraft failed to start: {e:?}</p><p>A browser with WebGPU or WebGL2 is required.</p>")),
            }
        }
    });
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

// //// Neoffice — added: whether the page's address has the parameter `name`, read by name with `URLSearchParams`
// as `host.rs` reads its own, where a substring test of the address found `show` in `showroom`.
fn has_param(name: &str) -> bool {
    web_sys::UrlSearchParams::new_with_str(&query()).is_ok_and(|params| params.has(name))
}

/// Wraps the app to read dropped files asynchronously (browsers can't read them synchronously)
/// and feed them through the inbox.
struct WebShell {
    app: SlideApp,
    inbox: Inbox,
    // //// Neoffice — the bridge to the page that frames the editor, when it asked for one.
    bridge: Option<host::Bridge>,
}

impl eframe::App for WebShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        // //// Neoffice — a hosting page opens the presentations: one dropped on the editor would replace the one the
        // page gave it, so it is not taken (a picture, a sound, a clip or an outline still is).
        let hosted = self.app.services.host_owns_documents;
        for f in dropped {
            let inbox = self.inbox.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
                // //// Neoffice — a presentation dropped on an editor whose page opens them (see above).
                if hosted && deckcraft_ui_egui::opens_as_presentation(&name) {
                    log::info!("{name} was dropped on an editor whose page opens the presentations: not taken");
                    return;
                }
                match f.bytes_async().await {
                    Ok(bytes) => {
                        inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                        ctx.request_repaint();
                    }
                    Err(e) => log::error!("couldn't read dropped file {name}: {e}"),
                }
            });
        }
        // //// Neoffice — what the hosting page refused to store shows before the frame is drawn.
        if let Some(bridge) = &self.bridge {
            bridge.before_frame(&mut self.app);
        }
        self.app.logic(ctx);
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
        // //// Neoffice — the hosting page hears when unsaved changes appear or go away.
        if let Some(bridge) = &self.bridge {
            bridge.after_frame(&mut self.app);
        }
    }
}

/// Web services: no file paths, so `pick_open`/`pick_save`/`read` stay `None` and the UI falls
/// back to `open_async` (browser picker → inbox) and `download`.
fn services(inbox: Inbox, ctx: egui::Context) -> Services {
    let open_inbox = inbox.clone();
    Services {
        open_async: Some(Box::new(move |purpose: &str| {
            let inbox = open_inbox.clone();
            let ctx = ctx.clone();
            let dialog = match purpose {
                "picture" => rfd::AsyncFileDialog::new().add_filter("Pictures", PICTURE_EXTS),
                "audio" => rfd::AsyncFileDialog::new().add_filter("Audio", AUDIO_EXTS),
                "video" => rfd::AsyncFileDialog::new().add_filter("Video", VIDEO_EXTS),
                _ => rfd::AsyncFileDialog::new().add_filter("Presentations", PRESENTATION_EXTS),
            };
            wasm_bindgen_futures::spawn_local(async move {
                let Some(file) = dialog.pick_file().await else {
                    return;
                };
                let bytes = file.read().await;
                inbox.lock().unwrap_or_else(|e| e.into_inner()).push((file.file_name(), bytes));
                ctx.request_repaint();
            });
        })),
        write: Some(Box::new(|name: &str, bytes: &[u8]| download(name, bytes))),
        download: Some(Box::new(|name: &str, bytes: &[u8]| {
            if let Err(e) = download(name, bytes) {
                log::error!("download of {name} failed: {e}");
            }
        })),
        inbox: Some(inbox),
        ..Default::default()
    }
}

/// Trigger a browser download of `bytes` named after the last component of `path`.
fn download(path: &str, bytes: &[u8]) -> Result<(), String> {
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "deckcraft".into());
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime_for(&name));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(&name);
    a.style().set_property("display", "none").map_err(js)?;
    let body = document.body().ok_or("no body")?;
    body.append_child(&a).map_err(js)?;
    a.click();
    a.remove();
    // Revoke after the click has been dispatched; the download keeps its own reference.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        web_sys::Url::revoke_object_url(&url).ok();
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000).map_err(js)?;
    Ok(())
}

fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("deckcraft") => "application/zip",
        _ => "application/octet-stream",
    }
}
