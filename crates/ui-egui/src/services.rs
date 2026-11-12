pub type ReadFn = Box<dyn Fn(&str) -> Result<Vec<u8>, String>>;

pub type WriteFn = Box<dyn FnMut(&str, &[u8]) -> Result<(), String>>;

pub type PickFn = Box<dyn FnMut(&str) -> Option<String>>;

/// `(purpose, request id)`. The host shows a picker and later pushes an [`InboxEvent`]
/// carrying that same id. Completing a cancelled or unknown id does nothing: the file is
/// not applied to whichever document happens to be active.
pub type OpenAsyncFn = Box<dyn FnMut(&str, u64)>;

pub type DownloadFn = Box<dyn FnMut(&str, &[u8])>;
// //// Neoffice — see `Services::host_save`. Upstream's v0.16 has a type of this name for its own hook (`(name, bytes, save_as)`, and its
// `SlideApp::save_to_host`); ours takes the presentation, so the page's rules for the name and the format stay with the bridge (`apps/deckcraft-web/src/host.rs`).
pub type HostSaveFn = Box<dyn FnMut(&mut deckcraft_engine::DocState) -> Result<(), String>>;
// //// Neoffice — added: whether a file is a presentation the editor would open (what a hosting page that opens the
// presentations does not let a drop bring in).
pub fn opens_as_presentation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".deckcraft", ".slidecraft", ".pptx", ".potx", ".ppsx"].iter().any(|e| lower.ends_with(e))
}

/// A file the host finished reading for the request `id` from [`SlideApp::begin_external_file`].
#[derive(Clone, Debug)]
pub struct InboxFile {
    /// Cancellation identity. A file whose id was cancelled, or never started, is ignored.
    pub id: u64,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What the host delivers through [`Inbox`] after an asynchronous read.
#[derive(Clone, Debug)]
pub enum InboxEvent {
    File(InboxFile),
    /// The picker was dismissed or the file could not be read. `id` will not be applied.
    Cancelled {
        id: u64,
    },
}

/// Files delivered asynchronously by the host (web file picker, dropped files).
pub type Inbox = std::sync::Arc<std::sync::Mutex<Vec<InboxEvent>>>;
// //// Neoffice — answers to the Nora pane's calls, delivered asynchronously by the host like `Inbox`.
pub type NoraInbox = std::sync::Arc<std::sync::Mutex<Vec<craft_nora::HttpResponse>>>;
// //// Neoffice — a host that connects the Nora pane makes these calls and delivers these answers.
pub use craft_nora::{HttpRequest, HttpResponse};

/// Platform services injected by the host (desktop or web).
#[derive(Default)]
pub struct Services {
    /// Browser-owned presenter popup; absent on native hosts.
    pub presenter: Option<Box<dyn crate::presenter::Host>>,
    /// Open-file dialog for a purpose (`open`, `picture`, `audio`, `video`) → path.
    pub pick_open: Option<PickFn>,
    /// Save dialog with a suggested name → path.
    pub pick_save: Option<PickFn>,
    pub read: Option<ReadFn>,
    pub write: Option<WriteFn>,
    /// Asynchronous open dialog (web). The callback receives the purpose and the request id
    /// from [`SlideApp::begin_external_file`]; the chosen file arrives through `inbox`.
    pub open_async: Option<OpenAsyncFn>,
    /// Hand bytes to the user as a named file (browser download).
    pub download: Option<DownloadFn>,
    // //// Neoffice — the browser's print dialog on a PDF (the web build: a hidden frame's `print()`), for the print Nora
    // opens on the presentation (« imprime-la »). File › Print keeps upstream's way (the PDF, downloaded on the web).
    /// Open the print dialog on these PDF bytes. An `Err` when nothing could be opened.
    pub print: Option<Box<dyn Fn(&[u8]) -> Result<(), String>>>,
    // //// Neoffice — added hook (no upstream equivalent), for a page that frames the editor.
    /// The page stores the presentation itself: Save and Save As give it the presentation instead of
    /// encoding a file for `download`. It marks the presentation saved once it holds it; an `Err` is
    /// shown in the status bar.
    pub host_save: Option<HostSaveFn>,
    // //// Neoffice — the page that frames the editor (the Drive of the Suite, Neodesk's window) opens the
    // presentations: New, Open, Close and Home are not offered and are refused, so the person is not shown a
    // presentation the page knows nothing about. The page's own file still opens. Off, as upstream has it, for
    // every other build.
    pub host_owns_documents: bool,
    // //// Neoffice — the connection of the Nora assistant pane. Nora (the agent) lives on the Neoffice server, so the
    // pane needs the host's session and credentials to reach her: the host makes the HTTP call and delivers the answer
    // through `nora_inbox`. Without one the pane says that Nora isn't available and offers no text box.
    /// Make one call for the Nora pane: a `POST` of a JSON body to a path of the Neoffice instance. The host adds the
    /// origin and the credentials of its session.
    pub nora_http: Option<Box<dyn Fn(craft_nora::HttpRequest)>>,
    /// The answers to [`Services::nora_http`], delivered asynchronously by the host.
    pub nora_inbox: Option<NoraInbox>,
    // //// Neoffice — the host opens a page of the person's instance for the pane (a card's Open, a link of a reply): the
    // path only (`/drive/f/…`), already checked by craft-nora (`instance_path`). Without it the pane shows a link as its words.
    /// Open a page of the person's instance, by its path, the host's own way.
    pub nora_open: Option<Box<dyn Fn(&str)>>,
    // //// Neoffice — the host carries a dictation to the server (`nora.api.stt.transcribe`, with the person's credentials):
    // the pane then offers its microphone, where the browser can record. Off for every other build.
    pub nora_dictation: bool,
    pub inbox: Option<Inbox>,
    /// Image on the system clipboard (PNG bytes), when the host can read one.
    pub clipboard_image: Option<Box<dyn FnMut() -> Option<Vec<u8>>>>,
    /// HTML on the system clipboard (Pages, Keynote, and other apps that write `public.html`).
    pub clipboard_html: Option<Box<dyn FnMut() -> Option<String>>>,
    /// Put HTML and its plain-text alternative on the system clipboard. `true` when it landed.
    pub clipboard_set_html: Option<Box<dyn FnMut(&str, &str) -> bool>>,
    /// Audio output for media playback (cpal on desktop); without one media plays silently.
    pub audio_out: Option<Box<dyn deckcraft_media::AudioOut>>,
    /// How many displays are connected, when the host can tell: Presenter View needs a second.
    pub displays: Option<Box<dyn FnMut() -> Option<usize>>>,
}
