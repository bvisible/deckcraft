// //// Neoffice — added file (no upstream equivalent): the bridge between the editor and the page that frames it.

//! The `postMessage` bridge to a same-origin parent page (`?host=parent`), for pages that embed
//! DeckCraft in an `<iframe>` (the Drive of the Suite, NeoDesk's window). It speaks the contract
//! upstream's GridCraft and PdfCraft already use, so one host page can drive every Craft editor.
//! Messages are plain objects with a `type`:
//!
//! - `deckcraft:ready` (to the parent): `{ version }`, the app is listening; send the file now.
//! - `deckcraft:open` (from the parent): `{ name, bytes }`, `bytes` an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a presentation dropped on the window: `name` is the file's name with
//!   its extension (`.pptx`, `.potx`, `.ppsx` or `.deckcraft`), which decides how it is read and what
//!   Save writes back.
//! - `deckcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`) for every file
//!   DeckCraft would otherwise download: Save and Save As, and the exports (PDF, PNG…). The parent
//!   stores it, or offers it, by the file's extension. Save and Save As hand the presentation back in
//!   the format it was opened in, under the name it came with: `x.pptx` (and `.potx`, `.ppsx`, each
//!   with its own content type) goes back as a PowerPoint file, `x.deckcraft` as a DeckCraft file. A presentation made in the editor
//!   is `<title>.pptx`.
//! - `deckcraft:saved` (from the parent): `{ name, ok, error }`. Neoffice's addition: how storing
//!   `name` went. `ok: false` marks the presentation unsaved again and says why in the status bar. A
//!   parent that never answers leaves the presentation saved once the file has been handed over, as
//!   upstream's bridges do.
//! - `deckcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `deckcraft:failed` (to the parent): `{ error }` when the app couldn't start, or couldn't open a file the
//!   page sent with `deckcraft:open` (no bytes to read, a name that isn't a presentation's, or a file the editor
//!   can't read). The editor then stays on what it had: the blank presentation, or the page's earlier file.
//!
//! Only the parent window on the page's own origin is heard or answered, and there is no bridge at
//! all unless a page of the same origin frames the editor. In host mode the page starts on a blank
//! presentation instead of the sample deck (the parent sends the file to edit, and the blank one
//! makes way for it), and `?author=<name>` is the name that signs comments.
//!
//! This file has two halves. [`Hosted`] and the functions around it are what the editor does for a
//! hosting page, with no browser type in them, so that their rules are tested natively. `bridge`
//! (WebAssembly only) carries them to and from the page.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use deckcraft_engine::cmd::file::save_bytes;
use deckcraft_engine::{DocState, Session};
use deckcraft_ui_egui::{HostSaveFn, SlideApp};

/// How many handed-over files are remembered while the page hasn't said how storing them went.
const MAX_UNANSWERED: usize = 32;

/// What the editor keeps for the page that frames it. Nothing here touches the browser.
#[derive(Default)]
struct Hosted {
    /// Files handed to the page as saves that it hasn't answered yet: (document uid, file name),
    /// oldest first.
    handed: RefCell<VecDeque<(u64, String)>>,
    /// Saves that failed, to show and to take back: (document uid if it was a save of a
    /// presentation, file name, why).
    failures: RefCell<Vec<(Option<u64>, String, String)>>,
    /// The blank presentation the editor started on until the page sends its own (document uid).
    placeholder: Cell<Option<u64>>,
    /// Files the page sent (`deckcraft:open`), opened at the start of the next frame: (file name, bytes).
    arrived: RefCell<Vec<(String, Vec<u8>)>>,
    /// Why files the page sent could not be opened, for the page (`deckcraft:failed`), before the next frame.
    open_failures: RefCell<Vec<String>>,
}

impl Hosted {
    /// The editor has just started on the presentation `app` holds, for want of a file from the page.
    fn started_on(&self, app: &SlideApp) {
        self.placeholder.set(app.session.active().map(|d| d.uid));
    }

    /// The `Services::host_save` of the editor: Save and Save As go to the page through `post`.
    fn save_service(self: &Rc<Self>, post: impl Fn(&str, &[u8]) -> Result<(), String> + 'static) -> HostSaveFn {
        let hosted = self.clone();
        Box::new(move |doc| hosted.save(doc, &post))
    }

    /// Hands `doc` to the page as a file (see [`save_target`]). Once the page holds it the presentation
    /// reads as saved, as it would after Save on the desktop, exactly as it is now: a change made
    /// after this call is not saved.
    fn save(&self, doc: &mut DocState, post: &dyn Fn(&str, &[u8]) -> Result<(), String>) -> Result<(), String> {
        let (name, format) = save_target(doc);
        let bytes = save_bytes(&doc.doc, format).map_err(|e| could_not_save(&name, &e.to_string()))?;
        post(&name, &bytes).map_err(|e| could_not_save(&name, &e))?;
        doc.saved_doc = doc.doc.clone();
        doc.revision = doc.revision.saturating_add(1);
        let mut handed = self.handed.borrow_mut();
        handed.push_back((doc.uid, name));
        while handed.len() > MAX_UNANSWERED {
            handed.pop_front();
        }
        Ok(())
    }

    /// The page says how storing `name` went. A refusal makes the presentation it came from unsaved
    /// again, and reaches the status bar, at the start of the next frame.
    fn answered(&self, name: &str, ok: bool, error: &str) {
        let uid = {
            let mut handed = self.handed.borrow_mut();
            let at = handed.iter().position(|(_, n)| n == name);
            at.and_then(|i| handed.remove(i)).map(|(uid, _)| uid)
        };
        if !ok {
            self.failures.borrow_mut().push((uid, name.to_string(), error.to_string()));
        }
    }

    /// The page sent a file to edit. It opens at the start of the next frame ([`Hosted::open_arrived`]).
    fn receive(&self, name: String, bytes: Vec<u8>) {
        self.arrived.borrow_mut().push((name, bytes));
    }

    /// The page sent `deckcraft:open` with nothing the editor can read as bytes. The page hears it before the next
    /// frame ([`Hosted::open_arrived`]).
    fn unreadable(&self, name: &str, why: &str) {
        self.open_failures.borrow_mut().push(could_not_open(name, why));
    }

    /// Before each frame: the files the page sent open, as a file dropped on the window does. They don't go through
    /// `SlideApp::run`, where a hosting page's editor refuses to open presentations itself. Returns what the page
    /// should hear went wrong (`deckcraft:failed`): bytes it didn't send, a name that isn't a presentation's (the
    /// editor would have inserted it into the presentation it has, as a dropped picture), a file the editor can't
    /// read. The person sees the same words in the status bar or the notice the editor shows.
    fn open_arrived(&self, app: &mut SlideApp) -> Vec<String> {
        let arrived = std::mem::take(&mut *self.arrived.borrow_mut());
        for (name, bytes) in arrived {
            if !deckcraft_ui_egui::opens_as_presentation(&name) {
                let why = could_not_open(&name, "it is not a presentation (.pptx, .potx, .ppsx or .deckcraft)");
                app.set_status(why.clone());
                self.open_failures.borrow_mut().push(why);
                continue;
            }
            let documents = app.session.documents().len();
            let toast = app.toast.as_ref().map(|t| (t.text.clone(), t.at));
            let status = app.status.clone();
            app.open_or_insert_bytes(&name, &bytes);
            if app.session.documents().len() > documents {
                continue;
            }
            // Not opened: the editor said why in a notice (`file.openBytes`) or in the status bar.
            let said = app
                .toast
                .as_ref()
                .filter(|t| toast.as_ref() != Some(&(t.text.clone(), t.at)))
                .map(|t| t.text.clone())
                .or_else(|| app.status.clone().filter(|now| status.as_ref() != Some(now)).map(|(message, _)| message));
            let why = said.unwrap_or_else(|| "the editor could not read it".to_string());
            self.open_failures.borrow_mut().push(could_not_open(&name, &why));
        }
        std::mem::take(&mut *self.open_failures.borrow_mut())
    }

    /// A file couldn't even be handed to the page (an export, for one): the status bar says so.
    fn failed(&self, name: &str, error: &str) {
        self.failures.borrow_mut().push((None, name.to_string(), error.to_string()));
    }

    /// Before each frame: what failed shows in the status bar, and a presentation the page refused
    /// to store reads as having unsaved changes again.
    fn settle(&self, app: &mut SlideApp) {
        let failures = std::mem::take(&mut *self.failures.borrow_mut());
        for (uid, name, error) in failures {
            if let Some(uid) = uid {
                mark_unsaved(&mut app.session, uid);
            }
            app.set_status(could_not_save(&name, &error));
        }
    }

    /// After each frame: the blank presentation makes way once a file is open. Returns whether any
    /// presentation has unsaved changes.
    fn after_frame(&self, app: &mut SlideApp) -> bool {
        if let Some(uid) = self.placeholder.get() {
            self.retire_placeholder(&mut app.session, uid);
        }
        app.session.documents().iter().any(|d| d.is_dirty())
    }

    /// The blank presentation goes once a file opened from bytes is there (the page's, or one the
    /// user picked), unless it was edited or closed meanwhile: then it is a presentation like any other.
    fn retire_placeholder(&self, session: &mut Session, uid: u64) {
        let docs = session.documents();
        let Some(at) = docs.iter().position(|d| d.uid == uid) else {
            self.placeholder.set(None);
            return;
        };
        if docs.get(at).is_some_and(|d| d.is_dirty()) {
            self.placeholder.set(None);
            return;
        }
        if !docs.iter().any(|d| d.opened_from.is_some()) {
            return;
        }
        let front = session.active().map(|d| d.uid);
        session.close_document(at);
        // Closing hands the front to the first presentation: give it back.
        if let Some(at) = front.and_then(|front| session.documents().iter().position(|d| d.uid == front)) {
            session.set_active(at);
        }
        self.placeholder.set(None);
    }
}

/// The name a presentation goes back under, and the engine's name for the format of that file. The
/// name it was opened from, when that is a presentation: a `.pptx` stays a `.pptx`, a `.potx` a
/// template and a `.ppsx` a show (the engine writes their own content types since v0.8), a
/// `.deckcraft` stays a `.deckcraft`. Any other presentation, one made in the editor, goes back as
/// `<title>.pptx`.
fn save_target(doc: &DocState) -> (String, &'static str) {
    if let Some((name, format)) = doc.opened_from.as_deref().and_then(origin_format) {
        let file = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().to_string());
        return (file.unwrap_or_else(|| name.to_string()), format);
    }
    (format!("{}.pptx", doc.title()), "pptx")
}

/// The engine's format for a file name that is a presentation DeckCraft writes back as it came.
fn origin_format(name: &str) -> Option<(&str, &'static str)> {
    let (_, ext) = name.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "pptx" => Some((name, "pptx")),
        "potx" => Some((name, "potx")),
        "ppsx" => Some((name, "ppsx")),
        "deckcraft" => Some((name, "deckcraft")),
        _ => None,
    }
}

/// `is_dirty` compares the two handles, so the same presentation under a new one reads as changed.
fn mark_unsaved(session: &mut Session, uid: u64) {
    let Some(at) = session.documents().iter().position(|d| d.uid == uid) else { return };
    let active = session.active_index();
    session.set_active(at);
    if let Some(doc) = session.active_mut() {
        doc.saved_doc = Arc::new((*doc.doc).clone());
    }
    if let Some(active) = active {
        session.set_active(active);
    }
}

/// `Couldn't open <name>: <why>`, without the name twice when the editor's own words start with it.
fn could_not_open(name: &str, why: &str) -> String {
    let why = why.strip_prefix(name).and_then(|rest| rest.strip_prefix(": ")).unwrap_or(why);
    format!("Couldn't open {name}: {why}")
}

fn could_not_save(name: &str, error: &str) -> String {
    format!("Couldn't save {name}: {error}")
}

#[cfg(target_arch = "wasm32")]
pub use bridge::Bridge;

/// The `postMessage` half: the page's side of the contract.
#[cfg(target_arch = "wasm32")]
mod bridge {
    use std::cell::Cell;

    use deckcraft_ui_egui::{Services, SlideApp};
    use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

    use super::{Hosted, Rc};

    /// The longest user name taken from `?author=`.
    const MAX_AUTHOR_CHARS: usize = 200;

    #[derive(Clone)]
    pub struct Bridge {
        window: web_sys::Window,
        parent: web_sys::Window,
        origin: String,
        hosted: Rc<Hosted>,
        /// The unsaved-changes state last reported to the parent.
        reported: Rc<Cell<Option<bool>>>,
    }

    impl Bridge {
        /// The bridge, when the page asked for it (`?host=parent`) and a page of its own origin
        /// frames it; `None` otherwise, and the editor then behaves as it always did.
        pub fn requested() -> Option<Self> {
            if query_param("host").as_deref() != Some("parent") {
                return None;
            }
            let window = web_sys::window()?;
            let parent = window.parent().ok().flatten()?;
            if js_sys::Object::is(&parent, &window) {
                return None;
            }
            // An opaque origin ("null") can't be addressed, and a parent whose origin can't be read
            // is on another one: neither is a host.
            let origin = window.location().origin().ok().filter(|o| o != "null")?;
            if parent.location().origin().ok().as_deref() != Some(origin.as_str()) {
                return None;
            }
            Some(Self { window, parent, origin, hosted: Rc::default(), reported: Rc::default() })
        }

        /// Sends every file the editor would download to the parent, hands it the presentations
        /// Save and Save As produce, and hears its files and its answers to the saves.
        pub fn connect(&self, services: &mut Services, ctx: egui::Context) {
            self.listen(ctx);
            let bridge = self.clone();
            services.download = Some(Box::new(move |name: &str, bytes: &[u8]| {
                if let Err(e) = bridge.post_save(name, bytes) {
                    log::error!("couldn't hand {name} to the page: {e}");
                    bridge.hosted.failed(name, &e);
                }
            }));
            let bridge = self.clone();
            services.host_save = Some(self.hosted.save_service(move |name, bytes| bridge.post_save(name, bytes)));
        }

        /// `?author=`: the host's name for the user, which signs comments.
        pub fn author() -> Option<String> {
            let author: String = query_param("author")?.trim().chars().take(MAX_AUTHOR_CHARS).collect();
            (!author.is_empty()).then_some(author)
        }

        /// The editor listens: the page can send the file. `app` holds the blank presentation the
        /// editor starts on until it does.
        pub fn ready(&self, app: &SlideApp) {
            self.hosted.started_on(app);
            self.send(&[("type", "deckcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
        }

        pub fn post_failed(&self, error: &str) {
            self.send(&[("type", "deckcraft:failed".into()), ("error", error.into())]);
        }

        /// Before each frame: the page's files open, and what it refused to store shows.
        pub fn before_frame(&self, app: &mut SlideApp) {
            for error in self.hosted.open_arrived(app) {
                log::error!("{error}");
                self.post_failed(&error);
            }
            self.hosted.settle(app);
        }

        /// After each frame: the page hears when unsaved changes appear or go away.
        pub fn after_frame(&self, app: &mut SlideApp) {
            let dirty = self.hosted.after_frame(app);
            if self.reported.get() != Some(dirty) && self.send(&[("type", "deckcraft:dirty".into()), ("dirty", dirty.into())]) {
                self.reported.set(Some(dirty));
            }
        }

        fn post_save(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
            let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
            let buffer = js_sys::Uint8Array::from(bytes).buffer();
            let message = message(&[("type", "deckcraft:save".into()), ("name", name.as_str().into()), ("bytes", buffer.clone().into())])
                .ok_or_else(|| "couldn't build the message".to_string())?;
            self.parent.post_message_with_transfer(&message, &self.origin, &js_sys::Array::of1(&buffer)).map_err(|e| reason(&e))
        }

        fn listen(&self, ctx: egui::Context) {
            let parent = self.parent.clone();
            let origin = self.origin.clone();
            let hosted = self.hosted.clone();
            let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                if event.origin() != origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &parent)) {
                    return;
                }
                let data = event.data();
                match string(&data, "type").as_deref() {
                    Some("deckcraft:open") => {
                        let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Presentation.pptx".to_string());
                        match bytes(&data) {
                            Some(bytes) => hosted.receive(name, bytes),
                            None => hosted.unreadable(&name, "the page sent no bytes to read (an ArrayBuffer or a Uint8Array)"),
                        }
                    }
                    Some("deckcraft:saved") => {
                        let name = string(&data, "name").unwrap_or_default();
                        let ok = js_sys::Reflect::get(&data, &JsValue::from_str("ok")).ok().and_then(|ok| ok.as_bool()) != Some(false);
                        let error = string(&data, "error").filter(|e| !e.trim().is_empty()).unwrap_or_else(|| "the page did not say why".to_string());
                        if !ok {
                            log::error!("the hosting page could not save {name}: {error}");
                        }
                        hosted.answered(&name, ok, &error);
                    }
                    _ => return,
                }
                ctx.request_repaint();
            });
            if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
                // The listener lives as long as the page.
                on_message.forget();
            }
        }

        /// Posts a message to the parent; false if it couldn't be sent.
        fn send(&self, fields: &[(&str, JsValue)]) -> bool {
            message(fields).is_some_and(|m| self.parent.post_message(&m, &self.origin).is_ok())
        }
    }

    fn query_param(key: &str) -> Option<String> {
        let search = web_sys::window()?.location().search().ok()?;
        web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(key)
    }

    fn reason(error: &JsValue) -> String {
        error.as_string().unwrap_or_else(|| format!("{error:?}"))
    }

    fn message(fields: &[(&str, JsValue)]) -> Option<js_sys::Object> {
        let m = js_sys::Object::new();
        for (key, value) in fields {
            js_sys::Reflect::set(&m, &JsValue::from_str(key), value).ok()?;
        }
        Some(m)
    }

    fn string(data: &JsValue, key: &str) -> Option<String> {
        js_sys::Reflect::get(data, &JsValue::from_str(key)).ok()?.as_string()
    }

    /// `bytes` as an `ArrayBuffer` or `Uint8Array`.
    fn bytes(data: &JsValue) -> Option<Vec<u8>> {
        let bytes = js_sys::Reflect::get(data, &JsValue::from_str("bytes")).ok()?;
        if let Some(array) = bytes.dyn_ref::<js_sys::Uint8Array>() {
            Some(array.to_vec())
        } else if bytes.is_instance_of::<js_sys::ArrayBuffer>() {
            Some(js_sys::Uint8Array::new(&bytes).to_vec())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use deckcraft_ui_egui::Services;
    use serde_json::json;

    use super::*;

    /// What the page received: (file name, bytes), in order.
    type Posted = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

    /// An editor wired as the browser wires it, with no presentation open, and what the page gets.
    fn editor() -> (SlideApp, Rc<Hosted>, Posted) {
        editor_with(|_, _| Ok(()))
    }

    fn editor_with(post: impl Fn(&str, &[u8]) -> Result<(), String> + 'static) -> (SlideApp, Rc<Hosted>, Posted) {
        let hosted = Rc::new(Hosted::default());
        let posted = Posted::default();
        let log = posted.clone();
        let service = hosted.save_service(move |name, bytes| {
            post(name, bytes)?;
            log.borrow_mut().push((name.to_string(), bytes.to_vec()));
            Ok(())
        });
        let services = Services { host_save: Some(service), ..Default::default() };
        (SlideApp::new(Session::new(), services), hosted, posted)
    }

    /// A presentation of three slides, as the page would send it, in `format`.
    fn deck(format: &str) -> Vec<u8> {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({})).unwrap();
        s.execute("slide.new", &json!({})).unwrap();
        save_bytes(&s.doc().unwrap().doc, format).unwrap()
    }

    fn active(app: &SlideApp) -> &DocState {
        app.session.active().unwrap()
    }

    fn unsaved(app: &SlideApp) -> bool {
        app.session.documents().iter().any(|d| d.is_dirty())
    }

    fn status(app: &SlideApp) -> Option<&str> {
        app.status.as_ref().map(|(msg, _)| msg.as_str())
    }

    fn holds(bytes: &[u8], part: &str) -> bool {
        bytes.windows(part.len()).any(|w| w == part.as_bytes())
    }

    /// A zip with the parts every PowerPoint package has (and none of DeckCraft's own).
    fn is_pptx_package(bytes: &[u8]) -> bool {
        bytes.starts_with(b"PK") && holds(bytes, "[Content_Types].xml") && holds(bytes, "ppt/presentation.xml") && !holds(bytes, "presentation.json")
    }

    /// A zip with DeckCraft's own parts.
    fn is_deckcraft_package(bytes: &[u8]) -> bool {
        bytes.starts_with(b"PK") && holds(bytes, "presentation.json") && holds(bytes, "mimetype") && !holds(bytes, "[Content_Types].xml")
    }

    #[test]
    fn a_presentation_opened_from_pptx_goes_back_as_pptx_under_the_same_name() {
        for name in ["Quarterly.pptx", "Quarterly.potx", "Quarterly.ppsx", "QUARTERLY.PPTX", "Q1 plan.v2.pptx"] {
            let (mut app, _, posted) = editor();
            app.open_or_insert_bytes(name, &deck("pptx"));
            assert_eq!(active(&app).opened_from.as_deref(), Some(name));
            app.save();
            let posted = posted.borrow();
            assert_eq!(posted.len(), 1, "{name}");
            let (sent_name, sent) = &posted[0];
            assert_eq!(sent_name, name);
            // A PowerPoint package (not DeckCraft's own zip), the engine's for this very presentation, as the
            // kind of file it came as: a template stays a template, a show a show...
            let doc = &active(&app).doc;
            assert!(is_pptx_package(sent) && !is_deckcraft_package(sent), "{name}");
            let kind = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
            assert_eq!(*sent, save_bytes(doc, &kind).unwrap(), "{name}");
            // ... and it opens again, with every slide.
            let again = deckcraft_engine::cmd::file::open_presentation(sent_name, sent).unwrap();
            assert_eq!(again.slides.len(), 3, "{name}");
        }
    }

    #[test]
    fn a_presentation_opened_from_deckcraft_goes_back_as_deckcraft() {
        let (mut app, _, posted) = editor();
        app.open_or_insert_bytes("Keynote.deckcraft", &deck("deckcraft"));
        app.save();
        let posted = posted.borrow();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].0, "Keynote.deckcraft");
        assert!(is_deckcraft_package(&posted[0].1) && !is_pptx_package(&posted[0].1));
        assert_eq!(posted[0].1, save_bytes(&active(&app).doc, "deckcraft").unwrap());
    }

    /// Without a hosting page the web build downloads the DeckCraft file, as upstream's does.
    #[test]
    fn without_a_page_save_still_downloads_the_deckcraft_file() {
        let downloads = Posted::default();
        let log = downloads.clone();
        let download = Box::new(move |name: &str, bytes: &[u8]| log.borrow_mut().push((name.to_string(), bytes.to_vec())));
        let mut app = SlideApp::new(Session::new(), Services { download: Some(download), ..Default::default() });
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        app.save();
        let downloads = downloads.borrow();
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].0, "Quarterly.deckcraft");
        assert!(is_deckcraft_package(&downloads[0].1));
        assert_eq!(downloads[0].1, save_bytes(&active(&app).doc, "deckcraft").unwrap());
    }

    /// With one, Save goes to the page and nothing is downloaded (the exports still are).
    #[test]
    fn with_a_page_save_does_not_download() {
        let downloads = Posted::default();
        let log = downloads.clone();
        let download = Box::new(move |name: &str, bytes: &[u8]| log.borrow_mut().push((name.to_string(), bytes.to_vec())));
        let handed = Posted::default();
        let sink = handed.clone();
        let host_save = Rc::new(Hosted::default()).save_service(move |name, bytes| {
            sink.borrow_mut().push((name.to_string(), bytes.to_vec()));
            Ok(())
        });
        let mut app = SlideApp::new(Session::new(), Services { download: Some(download), host_save: Some(host_save), ..Default::default() });
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        app.save();
        assert!(downloads.borrow().is_empty());
        assert_eq!(handed.borrow().len(), 1);
        assert!(!unsaved(&app));
    }

    #[test]
    fn a_presentation_made_in_the_editor_is_saved_as_pptx_under_its_title() {
        let (mut app, _, posted) = editor();
        app.run("file.new", json!({})).unwrap();
        app.save();
        let posted = posted.borrow();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].0, "Presentation1.pptx");
        assert!(is_pptx_package(&posted[0].1));
        assert_eq!(posted[0].1, save_bytes(&active(&app).doc, "pptx").unwrap());
    }

    #[test]
    fn a_name_that_is_not_a_presentation_is_not_kept() {
        assert_eq!(origin_format("a.pptx"), Some(("a.pptx", "pptx")));
        assert_eq!(origin_format("a.POTX"), Some(("a.POTX", "potx")));
        assert_eq!(origin_format("a.ppsx"), Some(("a.ppsx", "ppsx")));
        assert_eq!(origin_format("a.deckcraft"), Some(("a.deckcraft", "deckcraft")));
        for name in ["a.slidecraft", "outline.txt", "notes.md", "photo.png", "Presentation", "a.pptx.bak", "folder.pptx/a"] {
            assert_eq!(origin_format(name), None, "{name}");
        }
        // Such a presentation goes back as a PowerPoint file named after its title.
        let (mut app, _, posted) = editor();
        app.run("file.openBytes", json!({"name": "old.slidecraft", "data": deckcraft_engine::cmd::base64_encode(&deck("deckcraft"))})).unwrap();
        app.save();
        assert_eq!(posted.borrow()[0].0, "old.pptx");
    }

    #[test]
    fn the_page_gets_the_file_name_without_a_folder() {
        let (mut app, _, posted) = editor();
        app.open_or_insert_bytes("Team/Q1/Quarterly.pptx", &deck("pptx"));
        app.save();
        assert_eq!(posted.borrow()[0].0, "Quarterly.pptx");
    }

    #[test]
    fn a_presentation_counts_as_saved_exactly_as_it_was_handed_over() {
        let (mut app, _, _) = editor();
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        assert!(!unsaved(&app));
        app.run("slide.new", json!({})).unwrap();
        assert!(unsaved(&app));
        app.save();
        assert!(!unsaved(&app));
        // A change made after the save, even in the same frame, is not saved.
        app.run("slide.new", json!({})).unwrap();
        assert!(unsaved(&app));
    }

    #[test]
    fn a_save_the_page_refuses_makes_the_presentation_unsaved_again() {
        let (mut app, hosted, _) = editor();
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        app.save();
        assert!(!unsaved(&app));
        hosted.answered("Quarterly.pptx", false, "the folder is read-only");
        hosted.settle(&mut app);
        assert!(unsaved(&app));
        assert_eq!(status(&app), Some("Couldn't save Quarterly.pptx: the folder is read-only"));
        // Saving again hands it over again.
        app.save();
        assert!(!unsaved(&app));
        // An answer that says it went well changes nothing.
        hosted.answered("Quarterly.pptx", true, "");
        hosted.settle(&mut app);
        assert!(!unsaved(&app));
    }

    #[test]
    fn a_refusal_goes_to_the_presentation_that_was_saved() {
        let (mut app, hosted, _) = editor();
        app.open_or_insert_bytes("A.pptx", &deck("pptx"));
        app.open_or_insert_bytes("B.pptx", &deck("pptx"));
        let a = app.session.documents().iter().position(|d| d.opened_from.as_deref() == Some("A.pptx")).unwrap();
        app.session.set_active(a);
        app.save();
        let b = app.session.documents().iter().position(|d| d.opened_from.as_deref() == Some("B.pptx")).unwrap();
        app.session.set_active(b);
        hosted.answered("A.pptx", false, "no space left");
        hosted.settle(&mut app);
        let docs = app.session.documents();
        assert!(docs[a].is_dirty() && !docs[b].is_dirty());
        assert_eq!(app.session.active_index(), Some(b), "the focus stays where it was");
    }

    #[test]
    fn answers_are_matched_to_the_saves_in_order() {
        let (mut app, hosted, _) = editor();
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        app.save();
        app.run("slide.new", json!({})).unwrap();
        app.save();
        // The first save went through, the second didn't.
        hosted.answered("Quarterly.pptx", true, "");
        hosted.answered("Quarterly.pptx", false, "timeout");
        hosted.settle(&mut app);
        assert!(unsaved(&app));
        // Nothing is left to match another refusal to: it is shown, and takes nothing back.
        app.save();
        hosted.answered("Quarterly.pptx", true, "");
        hosted.answered("Quarterly.pptx", false, "late");
        hosted.settle(&mut app);
        assert!(!unsaved(&app));
        assert_eq!(status(&app), Some("Couldn't save Quarterly.pptx: late"));
    }

    #[test]
    fn a_file_that_cannot_be_handed_over_leaves_the_presentation_unsaved() {
        let (mut app, hosted, posted) = editor_with(|_, _| Err("the page is gone".to_string()));
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        app.run("slide.new", json!({})).unwrap();
        app.save();
        assert!(unsaved(&app));
        assert!(posted.borrow().is_empty());
        assert_eq!(status(&app), Some("Couldn't save Quarterly.pptx: the page is gone"));
        // An export that fails has no presentation to take back.
        hosted.failed("Quarterly.pdf", "the page is gone");
        hosted.settle(&mut app);
        assert_eq!(status(&app), Some("Couldn't save Quarterly.pdf: the page is gone"));
        assert!(unsaved(&app));
    }

    #[test]
    fn the_blank_presentation_makes_way_for_the_page_file() {
        let (mut app, hosted, _) = editor();
        app.run("file.new", json!({})).unwrap();
        hosted.started_on(&app);
        // Nothing sent yet: it stays, and is not an unsaved change.
        assert!(!hosted.after_frame(&mut app));
        assert_eq!(app.session.documents().len(), 1);
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        assert_eq!(app.session.documents().len(), 2);
        assert!(!hosted.after_frame(&mut app));
        assert_eq!(app.session.documents().len(), 1);
        assert_eq!(active(&app).opened_from.as_deref(), Some("Quarterly.pptx"));
        assert_eq!(active(&app).doc.slides.len(), 3);
    }

    #[test]
    fn the_page_file_opens_in_an_editor_whose_page_opens_the_presentations() {
        let (mut app, hosted, _) = editor();
        app.run("file.new", json!({})).unwrap();
        app.services.host_owns_documents = true;
        hosted.started_on(&app);
        // The editor itself may not open a presentation beside the page's...
        let data = deckcraft_engine::cmd::base64_encode(&deck("pptx"));
        assert!(app.run("file.openBytes", json!({"name": "Other.pptx", "data": data})).is_err());
        // ...but the file the page sends opens at the next frame, and the blank one makes way.
        hosted.receive("Quarterly.pptx".into(), deck("pptx"));
        assert_eq!(app.session.documents().len(), 1, "nothing opens before the frame");
        assert!(hosted.open_arrived(&mut app).is_empty(), "nothing went wrong");
        assert_eq!(app.session.documents().len(), 2);
        assert!(!hosted.after_frame(&mut app));
        assert_eq!(app.session.documents().len(), 1);
        assert_eq!(active(&app).opened_from.as_deref(), Some("Quarterly.pptx"));
        assert_eq!(active(&app).doc.slides.len(), 3);
        // Opened once: a second frame opens nothing more.
        assert!(hosted.open_arrived(&mut app).is_empty());
        assert_eq!(app.session.documents().len(), 1);
    }

    /// What the page sent and the editor could not open is said back to the page (`deckcraft:failed`), and the
    /// editor stays on what it had.
    #[test]
    fn a_file_the_page_sent_that_cannot_be_opened_is_said_to_the_page() {
        let (mut app, hosted, _) = editor();
        app.run("file.new", json!({})).unwrap();
        app.services.host_owns_documents = true;
        hosted.started_on(&app);
        let blank = active(&app).uid;
        // Bytes that are no presentation, under a presentation's name.
        hosted.receive("Broken.pptx".into(), b"not a zip at all".to_vec());
        let said = hosted.open_arrived(&mut app);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].starts_with("Couldn't open Broken.pptx: "), "{said:?}");
        // A file that is not a presentation is not inserted into the one the editor has.
        hosted.receive("photo.png".into(), deck("pptx"));
        let said = hosted.open_arrived(&mut app);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("photo.png") && said[0].contains("not a presentation"), "{said:?}");
        // No bytes at all.
        hosted.unreadable("Empty.pptx", "the page sent no bytes to read");
        assert_eq!(hosted.open_arrived(&mut app), vec!["Couldn't open Empty.pptx: the page sent no bytes to read".to_string()]);
        // Each is said once, and the editor is still on its blank presentation, unchanged.
        assert!(hosted.open_arrived(&mut app).is_empty());
        assert_eq!(app.session.documents().len(), 1);
        assert_eq!(active(&app).uid, blank);
        assert!(!unsaved(&app));
    }

    #[test]
    fn the_blank_presentation_stays_when_it_was_edited() {
        let (mut app, hosted, _) = editor();
        app.run("file.new", json!({})).unwrap();
        hosted.started_on(&app);
        app.run("slide.new", json!({})).unwrap();
        app.open_or_insert_bytes("Quarterly.pptx", &deck("pptx"));
        assert!(hosted.after_frame(&mut app), "the edited one has unsaved changes");
        assert_eq!(app.session.documents().len(), 2);
        // It is not the placeholder any more: a later file doesn't take it away either.
        app.open_or_insert_bytes("Other.pptx", &deck("pptx"));
        hosted.after_frame(&mut app);
        assert_eq!(app.session.documents().len(), 3);
    }

    #[test]
    fn the_presentation_in_front_stays_in_front_when_the_blank_one_goes() {
        let (mut app, hosted, _) = editor();
        app.run("file.new", json!({})).unwrap();
        hosted.started_on(&app);
        app.open_or_insert_bytes("A.pptx", &deck("pptx"));
        app.open_or_insert_bytes("B.pptx", &deck("pptx"));
        hosted.after_frame(&mut app);
        assert_eq!(app.session.documents().len(), 2);
        assert_eq!(active(&app).opened_from.as_deref(), Some("B.pptx"));
    }
}
