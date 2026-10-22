// //// Neoffice — added file (no upstream equivalent): the Nora assistant pane, built on craft-nora and craft-nora-ui.

//! The Nora pane: Neoffice's assistant, in a pane at the right of the window. It is the pane NeoWrite (WordCraft)
//! has, so that the same Nora works in every editor.
//!
//! Nora (the agent, her memory and her tools) lives on the Neoffice server. This pane is her chat window: it keeps a
//! [`NoraSession`] (the conversation, from `craft-nora`) and draws it with [`NoraPanel`] (from `craft-nora-ui`). It is
//! off until Preferences › Assistant or the `ui.nora` command turns it on; the browser build turns it on when the
//! page that hosts it offers Nora.
//!
//! The pane never opens a connection itself. The host makes the HTTP calls with its own session and credentials
//! ([`crate::Services::nora_http`]) and delivers the answers ([`crate::Services::nora_inbox`]). Without a connection
//! the pane says that Nora isn't available and offers no text box.
//!
//! What Nora asks for in the presentation (the `doc_*` actions) is played with DeckCraft's own commands, through
//! [`SlideApp::run`], the way its MCP tools play them:
//! - **Observe**: every slide's title and text in order, the current slide marked, and the slide count; the text cut at
//!   a length that fits her; the selection.
//! - **Execute** and **Batch**: DeckCraft's commands by id. Not those that open, save or close a presentation or the
//!   application, not those that are the person's (the interface: views, panes, zoom, dark mode, the slide show, this
//!   pane; their undo history; their clipboard; media playback), and none with a parameter that reaches a file of the
//!   computer.
//! - **SelectText**: the n-th occurrence of a text on the current slide (its shapes in order, then its notes).
//! - **TypeText**: into the text being edited, else into the one selected shape that holds text (its text replaced, as
//!   typing on a selected shape does), else into the slide's empty body placeholder, else into a new text box below the
//!   title, where the slide has the most free room.
//! - **RenderPage**: a slide as a picture (`file.render`); the page number is the slide number.
//! - **Save**: the app's `file.save` (for a presentation without a file of its own, its Save As: the hosting page, a
//!   dialog, or a download).
//! - **Undo**: takes back Nora's last change whole, unless the presentation changed since.
//!
//! An action that fails leaves the presentation as it was, its undo history included, and what Nora changed can be
//! undone a whole action at a time.

use std::sync::Arc;

use craft_nora::{
    AppKind, CommandCall, CommandPolicy, DocActionKind, DocumentCall, DocumentInfo, HttpRequest, HttpResponse, Input, NoraError, NoraSession, Output,
    PageContext, Selection, SessionConfig,
};
use craft_nora_ui::{NoraLabels, NoraPanel, NoraStyle, PanelAction, drive};
use deckcraft_engine::{DocState, History, Target};
use deckcraft_model::{PhType, Presentation, Shape, ShapeId, ShapeKind, TextBody};
use egui::{CornerRadius, Sense, Ui, vec2};
use serde_json::{Value, json};

use crate::SlideApp;
use crate::icons::{self, Icon};
use crate::neoffice_i18n::{Lang, fmt, tr};
use crate::theme::{self, Tokens};

/// The most characters of the selection told to Nora with a message; she reads the presentation herself when she needs
/// more.
const MAX_SELECTION_CHARS: usize = 2_000;

/// The most characters of the presentation told to Nora when she reads it (as in WordCraft).
const MAX_OBSERVED_CHARS: usize = 50_000;

/// Parameter names that would reach a file of the computer. A command given one is refused. (`dir` is a direction for
/// DeckCraft: Delete Text, Text Direction, Distribute, a shadow's; its commands read a file by `path` only.)
const FILE_KEYS: &[&str] = &["path", "paths", "file", "files", "filename", "folder", "directory"];

/// Engine commands that are the person's rather than the presentation's: the slide show and the switch to another open
/// presentation (the interface); their undo history (`edit.undo` and `edit.redo` take back the person's own work, and
/// the pane's Undo would then take back theirs as Nora's); their clipboard (`edit.copy`, `edit.cut`, and `edit.paste`
/// with `edit.pasteText` and `edit.pasteSpecial`: every open presentation shares it, so Nora would read what they copied
/// elsewhere); and the playback
/// of a presentation's media, which decodes a file's audio or video (`media.play`, `media.seek`, `media.toggle`, and
/// `media.posterFrame`, which decodes a frame): only the person's click does that.
const THE_PERSON_S: &[&str] = &[
    "show.fromStart",
    "show.fromCurrent",
    "window.",
    "edit.undo",
    "edit.redo",
    "edit.copy",
    "edit.cut",
    "edit.paste",
    "media.play",
    "media.seek",
    "media.toggle",
    "media.posterFrame",
];

/// The height of a new text box of Nora's, before it grows to its text.
const NEW_BOX_HEIGHT: f64 = 36.0;

/// The room left above a new text box in the free strip it goes in.
const BAND_GAP: f64 = 12.0;

/// How deep groups are read (DeckCraft's own walks stop there too).
const MAX_DEPTH: usize = 64;

const NO_PRESENTATION: &str = "no presentation is open";

/// What Nora may play in a presentation: the editing commands. Not those that open, save or close a presentation or
/// the application (the default policy), and not those that belong to the person: the interface, Neoffice's (`ui.`,
/// this pane) and DeckCraft's views, panes, zoom, ribbon and dark mode ([`crate::UI_COMMANDS`]), and the others in
/// [`THE_PERSON_S`] (the slide show, their undo history, their clipboard, media playback).
fn policy() -> CommandPolicy {
    let theirs = crate::UI_COMMANDS.iter().map(|c| c.0).chain(THE_PERSON_S.iter().copied());
    theirs.fold(CommandPolicy::default().deny_prefix("ui."), |policy, id| policy.deny_prefix(id))
}

/// What the pane keeps between frames: the conversation, the panel that draws it, and what Nora changed.
pub struct NoraState {
    session: NoraSession,
    panel: NoraPanel,
    /// The changes of Nora's that Undo can take back, the newest last: one per change the conversation counts.
    changes: Vec<Change>,
    /// Nora's actions that wait for the pointer to let go of the presentation (see [`pointer_busy`]).
    waiting: Vec<DocumentCall>,
    /// The language of the pane's words: the person's (see [`Lang::system`]).
    pub lang: Lang,
}

/// One change Nora made: in which presentation, how many undo steps it took, and the presentation as she left it.
struct Change {
    uid: u64,
    steps: usize,
    after: Arc<Presentation>,
}

impl Default for NoraState {
    fn default() -> Self {
        let context = PageContext::editor(AppKind::Deck, DocumentInfo::named(""));
        let config = SessionConfig { policy: policy(), ..SessionConfig::default() };
        NoraState {
            session: NoraSession::new(config, context),
            panel: NoraPanel::new(),
            changes: Vec::new(),
            waiting: Vec::new(),
            lang: Lang::system(),
        }
    }
}

/// The pane at the right of the window, with the title row of DeckCraft's task panes ("Nora" and a close button), then
/// the conversation.
pub fn pane(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("nora_pane")
        .resizable(true)
        .default_size(320.0)
        .size_range(260.0..=560.0)
        .frame(egui::Frame::NONE.fill(t.card).inner_margin(egui::Margin::same(10)))
        .show(ui, |ui| {
            if header(ui, &t) {
                let _ = app.run("ui.nora", json!({"on": false}));
                return;
            }
            show(app, ui);
        });
}

/// The title row of DeckCraft's task panes: the pane's name, and a close button. True when the button was clicked.
fn header(ui: &mut Ui, t: &Tokens) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        // Nora is a name, the same in every language.
        ui.label(egui::RichText::new("Nora").font(theme::bold(16.0)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
            }
            icons::paint(ui.painter(), r.shrink(3.0), Icon::Close, t.text_dim, false);
            close = resp.clicked();
        });
    });
    ui.add_space(6.0);
    close
}

/// Draws the pane's content (inside the pane's frame) and moves the conversation along.
pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    let mut outputs = incoming(app);
    outputs.extend(drive(ui.ctx(), &mut app.nora.session));
    let lang = app.nora.lang;
    app.nora.panel.labels = labels(lang);
    // The panel takes its words for a failure of the connection once, when it is made: given again each frame, as the
    // labels, so that they follow the pane's language.
    app.nora.panel = std::mem::take(&mut app.nora.panel).with_error_words(error_worder(lang));
    app.nora.panel.style = style(ui);
    app.nora.panel.offline = app.services.nora_http.is_none().then(|| tr(lang, "Nora isn't available in this version yet").to_string());
    for action in app.nora.panel.show(ui, &app.nora.session) {
        match &action {
            PanelAction::Send(_) => {
                // Where the user is, as of the message: told to Nora with it, never read on every frame.
                let context = context_of(app);
                outputs.extend(app.nora.session.handle(Input::SetContext(context)));
            }
            // A new conversation can't undo the old one's changes, and what the old one asked for no longer runs.
            PanelAction::NewConversation => {
                app.nora.changes.clear();
                app.nora.waiting.clear();
            }
            _ => {}
        }
        outputs.extend(app.nora.session.handle(action.into_input()));
    }
    perform(app, outputs, ui.ctx());
    if !app.nora.waiting.is_empty() {
        ui.ctx().request_repaint();
    }
}

/// What the conversation has for this frame: what the host's answers ask for, and the actions that waited for the
/// pointer.
fn incoming(app: &mut SlideApp) -> Vec<Output> {
    // The older ones first: actions play in the order Nora asked for them.
    let mut outputs = released(app);
    outputs.extend(deliver_answers(app));
    outputs
}

/// The answers the host delivered since the last frame, fed to the conversation.
fn deliver_answers(app: &mut SlideApp) -> Vec<Output> {
    let Some(inbox) = app.services.nora_inbox.clone() else { return Vec::new() };
    let answers = std::mem::take(&mut *inbox.lock().unwrap_or_else(|e| e.into_inner()));
    answers.into_iter().flat_map(|answer| app.nora.session.handle(Input::Http(answer))).collect()
}

/// Whether the pointer is in the middle of changing the presentation: a drag on the slide (moving or resizing
/// shapes, or a tool's: drawing a shape, a text box, ink or a freeform, selecting text, a guide), of a slide in the
/// thumbnails or the slide sorter, of a whole section in the sorter, or of an object's row in the Selection pane.
/// Nora's actions wait for it, as craft-nora's host contract has them wait for the person's gesture: played now, they
/// would become part of that gesture and its one undo step, change the text or the shapes it points at, or move the
/// slides, the object or the guide it drags by their index.
fn pointer_busy(app: &SlideApp) -> bool {
    app.thumb_drag.is_some()
        || app.sorter.drag.is_some()
        || app.selpane.drag.is_some()
        || app.session.tool.dragging()
        || app.session.tool.guide_drag.is_some()
        || app.session.active().is_some_and(|d| d.interaction.is_some())
}

/// The actions that waited for the pointer, once it let go.
fn released(app: &mut SlideApp) -> Vec<Output> {
    if pointer_busy(app) {
        return Vec::new();
    }
    std::mem::take(&mut app.nora.waiting).into_iter().map(Output::Document).collect()
}

/// Does what the conversation asks: a call goes to the host, a document action is played (or waits for the pointer), a
/// change repaints. The results and the missing-connection answers come back as inputs and may ask for more, so this
/// goes round a few times at most.
fn perform(app: &mut SlideApp, outputs: Vec<Output>, ctx: &egui::Context) {
    let mut queue = outputs;
    for _ in 0..8 {
        if queue.is_empty() {
            return;
        }
        let mut next = Vec::new();
        for output in std::mem::take(&mut queue) {
            match output {
                Output::Http(request) => next.extend(send(app, request)),
                // Behind the ones that already wait, too, so that they keep their order.
                Output::Document(call) if pointer_busy(app) || !app.nora.waiting.is_empty() => app.nora.waiting.push(call),
                Output::Document(call) => {
                    let result = run_document(app, &call.kind);
                    next.extend(app.nora.session.handle(Input::DocumentResult { id: call.id, result }));
                }
                Output::Changed => ctx.request_repaint(),
            }
        }
        queue = next;
    }
}

/// Plays one action of Nora in the open presentation. One that changes it is played against a snapshot: if it fails,
/// the presentation, its selection and its undo history go back to the snapshot, so a failed action leaves it as it
/// was; if it succeeds, the change is remembered with the undo steps it took.
fn run_document(app: &mut SlideApp, kind: &DocActionKind) -> Result<Value, String> {
    let changes = kind.modifies() || matches!(kind, DocActionKind::SelectText { .. } | DocActionKind::Undo);
    if !changes {
        return apply(app, kind, &mut 0);
    }
    let Some(before) = Snapshot::take(app) else { return Err(NO_PRESENTATION.into()) };
    let mut steps = 0;
    let mut result = apply(app, kind, &mut steps);
    if result.is_err() {
        before.restore(app);
        return result;
    }
    if kind.modifies() {
        let after = app.session.documents().iter().find(|d| d.uid == before.uid).map_or_else(|| Arc::clone(&before.doc), |d| Arc::clone(&d.doc));
        // An action that changed nothing (no undo step, the same presentation) says so, and leaves nothing to undo: the
        // conversation then keeps nothing to undo for it either (craft-nora from 9d28340, which `Cargo.toml` pins). A
        // list (a batch's results) can't carry it: kept with no step, as the conversation keeps it.
        let unchanged = steps == 0 && Arc::ptr_eq(&after, &before.doc) && result.as_mut().is_ok_and(say_unchanged);
        if !unchanged {
            app.nora.changes.push(Change { uid: before.uid, steps, after });
        }
    }
    result
}

/// Marks an answer as having changed nothing (`"changed": false`) when it can carry the mark: an object, or no answer
/// at all, which becomes `{"changed": false}`. A list (a batch's results, a gallery) can't.
fn say_unchanged(value: &mut Value) -> bool {
    match value {
        Value::Object(map) => {
            map.insert("changed".into(), Value::Bool(false));
            true
        }
        Value::Null => {
            *value = json!({"changed": false});
            true
        }
        _ => false,
    }
}

/// The open presentation as it was before an action of Nora's.
struct Snapshot {
    uid: u64,
    doc: Arc<Presentation>,
    selection: deckcraft_engine::Selection,
    history: History,
}

impl Snapshot {
    fn take(app: &SlideApp) -> Option<Snapshot> {
        let d = app.session.active()?;
        Some(Snapshot { uid: d.uid, doc: Arc::clone(&d.doc), selection: d.selection.clone(), history: d.history.clone() })
    }

    /// Back to the presentation the action started in, as it was then. Its revision moves on, so that everything drawn
    /// from it is drawn again.
    fn restore(self, app: &mut SlideApp) {
        if let Some(at) = app.session.documents().iter().position(|d| d.uid == self.uid) {
            app.session.set_active(at);
        }
        if let Some(d) = app.session.active_mut().filter(|d| d.uid == self.uid) {
            d.doc = self.doc;
            d.selection = self.selection;
            d.history = self.history;
            d.revision = d.revision.saturating_add(1);
        }
    }
}

fn apply(app: &mut SlideApp, kind: &DocActionKind, steps: &mut usize) -> Result<Value, String> {
    match kind {
        DocActionKind::Observe => observe(app),
        DocActionKind::Execute { command, params } => execute(app, command, params, steps),
        DocActionKind::Batch { commands } => batch(app, commands, steps),
        DocActionKind::SelectText { text, occurrence } => select_text(app, text, *occurrence),
        DocActionKind::TypeText { text, paragraphs } => type_text(app, text, *paragraphs, steps),
        DocActionKind::RenderPage { page, scale } => render_slide(app, *page, *scale),
        DocActionKind::Save => save(app),
        DocActionKind::Undo => undo(app),
    }
}

/// Runs one of DeckCraft's commands for Nora, through [`SlideApp::run`] as its MCP tools do, and counts the undo step it
/// left. The engine pushes one for each command that changed the presentation, and that step holds the presentation from
/// before the command: counting this way stays right when the undo history is full and drops its oldest step.
fn run_command(app: &mut SlideApp, id: &str, params: Value, steps: &mut usize) -> Result<Value, String> {
    let before = app.session.active().map(|d| (d.uid, Arc::clone(&d.doc)));
    let result = app.run(id, params);
    let stepped = before.is_some_and(|(uid, doc)| {
        app.session
            .active()
            .is_some_and(|d| d.uid == uid && !Arc::ptr_eq(&d.doc, &doc) && d.history.undo.last().is_some_and(|step| Arc::ptr_eq(&step.doc, &doc)))
    });
    if stepped {
        *steps += 1;
    }
    result
}

/// One command of Nora's, unless it would reach a file of the computer.
fn execute(app: &mut SlideApp, command: &str, params: &Value, steps: &mut usize) -> Result<Value, String> {
    if let Some(key) = file_key(params) {
        return Err(format!("`{command}` is refused: `{key}` would reach a file of the computer"));
    }
    run_command(app, command, params.clone(), steps)
}

/// The first parameter name, at any depth, that would reach a file.
fn file_key(params: &Value) -> Option<String> {
    match params {
        Value::Object(map) => map
            .iter()
            .find_map(|(key, value)| if FILE_KEYS.contains(&key.to_ascii_lowercase().as_str()) { Some(key.clone()) } else { file_key(value) }),
        Value::Array(list) => list.iter().find_map(file_key),
        _ => None,
    }
}

/// Several commands in order. The first that fails ends the batch with an error, and the caller takes back the ones
/// that ran.
fn batch(app: &mut SlideApp, commands: &[CommandCall], steps: &mut usize) -> Result<Value, String> {
    let mut results = Vec::new();
    for call in commands {
        match execute(app, &call.command, &call.params, steps) {
            Ok(value) => results.push(json!({"command": call.command, "ok": true, "result": value})),
            Err(error) => return Err(format!("`{}` failed: {error}", call.command)),
        }
    }
    Ok(Value::Array(results))
}

/// What Nora learns when she reads the presentation: its name, its slide count, which slide is current, every slide's
/// title and text in order (cut at a length that fits her), the slides' word count, and the selection.
fn observe(app: &mut SlideApp) -> Result<Value, String> {
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    let count = d.doc.slides.len();
    let current = if count == 0 { 0 } else { d.selection.slide + 1 };
    let (text, words) = outline(&d.doc, d.selection.slide);
    let shown: String = text.chars().take(MAX_OBSERVED_CHARS).collect();
    let truncated = shown.len() < text.len();
    let name = name_of(d);
    let selection = selection_of(app);
    Ok(json!({
        "name": name,
        "slides": count,
        "pages": count,
        "currentSlide": current,
        "words": words,
        "text": shown,
        "textTruncated": truncated,
        "selection": selection,
    }))
}

/// The presentation as text, slide by slide, a blank line between two: `Slide 2 (current): <its title>`, then the rest
/// of its text shape by shape (a table row by row, its cells between ` | `), then `Notes: <its notes>`. Hidden slides
/// are marked; hidden shapes show nothing. Also the word count of the slides' own text (titles and shapes, not notes).
fn outline(doc: &Presentation, current: usize) -> (String, usize) {
    let mut slides = Vec::with_capacity(doc.slides.len());
    let mut words = 0;
    for (index, slide) in doc.slides.iter().enumerate() {
        let marks: Vec<&str> =
            [(index == current, "current"), (slide.hidden, "hidden")].into_iter().filter(|(on, _)| *on).map(|(_, mark)| mark).collect();
        let marks = if marks.is_empty() { String::new() } else { format!(" ({})", marks.join(", ")) };
        let title_at = slide.shapes.iter().position(|s| !s.hidden && s.ph_type().is_some_and(PhType::is_title));
        let title =
            title_at.and_then(|at| slide.shapes.get(at)).and_then(|s| s.text.as_ref()).map(|t| plain(t).replace('\n', " ")).unwrap_or_default();
        let title = title.trim();
        words += title.split_whitespace().count();
        let mut lines = vec![if title.is_empty() { format!("Slide {}{marks}:", index + 1) } else { format!("Slide {}{marks}: {title}", index + 1) }];
        let mut texts = Vec::new();
        for (at, shape) in slide.shapes.iter().enumerate() {
            if Some(at) != title_at {
                texts_of(shape, &mut texts, 0);
            }
        }
        words += texts.iter().map(|text| text.split_whitespace().count()).sum::<usize>();
        lines.extend(texts);
        let notes = plain(&slide.notes);
        if !notes.trim().is_empty() {
            lines.push(format!("Notes: {}", notes.trim()));
        }
        slides.push(lines.join("\n"));
    }
    (slides.join("\n\n"), words)
}

/// A text body as plain text: a line per paragraph, and a line break inside one as a new line too.
fn plain(body: &TextBody) -> String {
    body.text().replace('\u{b}', "\n")
}

/// The text a shape shows, in reading order: its own; a table's row by row; a group's shape by shape. A hidden shape
/// shows none.
fn texts_of(shape: &Shape, out: &mut Vec<String>, depth: usize) {
    if shape.hidden || depth > MAX_DEPTH {
        return;
    }
    match &shape.kind {
        ShapeKind::Table(table) => {
            for row in &table.rows {
                let cells: Vec<String> = row.cells.iter().map(|cell| plain(&cell.text).replace('\n', " ")).collect();
                if cells.iter().any(|cell| !cell.trim().is_empty()) {
                    out.push(cells.join(" | "));
                }
            }
        }
        ShapeKind::Group { children, .. } => {
            for child in children {
                texts_of(child, out, depth + 1);
            }
        }
        _ => {
            if let Some(text) = shape.text.as_ref().map(plain).filter(|text| !text.trim().is_empty()) {
                out.push(text);
            }
        }
    }
}

/// The presentation's name as the person knows it: its file's (the one it was saved to, or the one the hosting page
/// opened it from), else its title.
fn name_of(d: &DocState) -> String {
    let file = |name: &str| std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().to_string());
    d.path.as_deref().and_then(file).or_else(|| d.opened_from.as_deref().and_then(file)).unwrap_or_else(|| d.title())
}

/// The selected text: the range being edited, else the text of the selected shapes, which typing and formatting act on.
/// Empty when nothing is selected, or for a bare caret.
fn selected_text(d: &DocState) -> String {
    if let Some(t) = &d.selection.text {
        let (a, b) = t.ordered();
        return match deckcraft_engine::cmd::text::body_of(d, t) {
            Some(body) if t.is_range() => deckcraft_model::edit::text_range(body, a, b).replace('\u{b}', "\n"),
            _ => String::new(),
        };
    }
    let mut texts = Vec::new();
    for shape in d.selected_shapes() {
        texts_of(shape, &mut texts, 0);
    }
    texts.join("\n")
}

/// The selection as Nora reads it: the current slide, the selected shapes, the text being edited and where, the
/// selected text, and its formatting (`format.state`).
fn selection_of(app: &mut SlideApp) -> Value {
    let Some(d) = app.session.active() else { return Value::Null };
    let slide = if d.doc.slides.is_empty() { 0 } else { d.selection.slide + 1 };
    let text: String = selected_text(d).chars().take(MAX_OBSERVED_CHARS).collect();
    let shapes = d.selection.shapes.clone();
    let editing = d.selection.text.as_ref().map(|t| {
        let mut at = json!({"anchor": [t.anchor.0, t.anchor.1], "caret": [t.caret.0, t.caret.1]});
        if let Some(map) = at.as_object_mut() {
            if t.notes {
                map.insert("notes".into(), json!(true));
            } else {
                map.insert("shape".into(), json!(t.shape));
            }
            if let Some((row, col)) = t.cell {
                map.insert("cell".into(), json!([row, col]));
            }
        }
        at
    });
    let anything = editing.is_some() || !shapes.is_empty();
    let format = if anything { app.run("format.state", json!({})).ok() } else { None };
    json!({"slide": slide, "shapes": shapes, "text": text, "editing": editing, "format": format})
}

/// Where a text was found: in a shape (a table's cell), or in the slide's notes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    Shape { id: ShapeId, cell: Option<(usize, usize)> },
    Notes,
}

/// A text found on the slide: where, and from where to where (paragraph, character).
#[derive(Debug, PartialEq)]
struct Found {
    place: Place,
    from: (usize, usize),
    to: (usize, usize),
}

/// The text bodies of the current slide in reading order (or of the master or layout being edited): its shapes, a
/// table cell by cell, a group shape by shape, hidden ones left out; then the slide's notes.
fn text_places(d: &DocState) -> Vec<(Place, &TextBody)> {
    fn walk<'a>(shape: &'a Shape, out: &mut Vec<(Place, &'a TextBody)>, depth: usize) {
        if shape.hidden || depth > MAX_DEPTH {
            return;
        }
        match &shape.kind {
            ShapeKind::Table(table) => {
                for (r, row) in table.rows.iter().enumerate() {
                    for (c, cell) in row.cells.iter().enumerate() {
                        out.push((Place::Shape { id: shape.id, cell: Some((r, c)) }, &cell.text));
                    }
                }
            }
            ShapeKind::Group { children, .. } => {
                for child in children {
                    walk(child, out, depth + 1);
                }
            }
            ShapeKind::Shape | ShapeKind::Connector { .. } => {
                if let Some(body) = &shape.text {
                    out.push((Place::Shape { id: shape.id, cell: None }, body));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in d.shapes() {
        walk(shape, &mut out, 0);
    }
    if d.selection.target == Target::Slides
        && let Some(slide) = d.current_slide()
    {
        out.push((Place::Notes, &slide.notes));
    }
    out
}

/// The `occurrence`-th time `needle` appears on the current slide, within a paragraph, as written (WordCraft's
/// `select.text` finds it the same way). `Err` holds how many times it appears.
fn find_text(d: &DocState, needle: &str, occurrence: u32) -> Result<Found, usize> {
    let wanted = usize::try_from(occurrence).unwrap_or(usize::MAX).max(1);
    let mut seen = 0;
    for (place, body) in text_places(d) {
        for (p, paragraph) in body.paragraphs.iter().enumerate() {
            let text = paragraph.text();
            for (at, _) in text.match_indices(needle) {
                seen += 1;
                if seen == wanted {
                    let from = text.get(..at).map_or(0, |before| before.chars().count());
                    return Ok(Found { place, from: (p, from), to: (p, from + needle.chars().count()) });
                }
            }
        }
    }
    Err(seen)
}

/// Selects the `occurrence`-th time `needle` appears on the current slide (`text.edit`, then `text.select`).
fn select_text(app: &mut SlideApp, needle: &str, occurrence: u32) -> Result<Value, String> {
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    if d.doc.slides.is_empty() && d.selection.target == Target::Slides {
        return Err("the presentation has no slides".into());
    }
    let slide = d.selection.slide + 1;
    let found = find_text(d, needle, occurrence).map_err(|seen| match seen {
        0 => format!("`{needle}` is not on slide {slide}"),
        _ => format!("`{needle}` is on slide {slide} {seen} time(s), not {occurrence}"),
    })?;
    let (anchor, caret) = (json!([found.from.0, found.from.1]), json!([found.to.0, found.to.1]));
    let (edit, place) = match found.place {
        Place::Shape { id, cell: None } => (json!({"id": id, "at": anchor}), json!({"shape": id})),
        Place::Shape { id, cell: Some((row, col)) } => {
            (json!({"id": id, "at": anchor, "cell": [row, col]}), json!({"shape": id, "cell": [row, col]}))
        }
        Place::Notes => (json!({"notes": true}), json!({"notes": true})),
    };
    app.run("text.edit", edit)?;
    app.run("text.select", json!({"anchor": anchor, "caret": caret}))?;
    Ok(merged(json!({"slide": slide, "anchor": anchor, "caret": caret, "text": needle}), place))
}

/// `value` with the members of `more` added (both objects).
fn merged(mut value: Value, more: Value) -> Value {
    if let (Some(map), Value::Object(more)) = (value.as_object_mut(), more) {
        map.extend(more);
    }
    value
}

/// Where Nora's text goes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TypeInto {
    /// The text being edited, at its caret.
    Caret,
    /// The one selected shape that holds text: its text is replaced.
    Shape(ShapeId),
    /// A new text box on the current slide.
    NewBox,
}

/// Types Nora's text: into the text being edited (replacing its selection), else into the one selected shape that
/// holds text (replacing its text, as typing on a selected shape does), else into the slide's empty body placeholder,
/// else into a new text box on the current slide, below its title where the most room is free ([`new_box_rect`]).
/// With `paragraphs` each line is a paragraph; without, the lines are lines of one paragraph. Her words are typed as
/// she wrote them: AutoCorrect and smart quotes, which change the user's typing, are off while she types.
fn type_text(app: &mut SlideApp, text: &str, paragraphs: bool, steps: &mut usize) -> Result<Value, String> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let text = if paragraphs { text } else { text.replace('\n', "\u{b}") };
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    let into = if d.selection.text.is_some() {
        TypeInto::Caret
    } else {
        match d.selected_shapes().as_slice() {
            [shape] if matches!(shape.kind, ShapeKind::Shape | ShapeKind::Connector { .. }) => TypeInto::Shape(shape.id),
            _ => empty_body(d).map_or(TypeInto::NewBox, TypeInto::Shape),
        }
    };
    if into == TypeInto::NewBox {
        if d.doc.slides.is_empty() && d.selection.target == Target::Slides {
            return Err("the presentation has no slide to type on".into());
        }
        if text.is_empty() {
            return Err("there is no text to type, and no text box is selected".into());
        }
    }
    let rect = new_box_rect(d);
    let prefs = (app.session.prefs.autocorrect, app.session.prefs.smart_quotes);
    app.session.prefs.autocorrect = false;
    app.session.prefs.smart_quotes = false;
    let result = type_into(app, &text, into, rect, steps);
    (app.session.prefs.autocorrect, app.session.prefs.smart_quotes) = prefs;
    result
}

fn type_into(app: &mut SlideApp, text: &str, into: TypeInto, rect: Value, steps: &mut usize) -> Result<Value, String> {
    match into {
        TypeInto::Caret => {}
        TypeInto::Shape(id) => {
            run_command(app, "text.edit", json!({"id": id}), steps)?;
            run_command(app, "text.selectAll", json!({}), steps)?;
        }
        // An empty new text box is left with the caret in it.
        TypeInto::NewBox => {
            run_command(app, "insert.textBox", json!({"rect": rect}), steps)?;
        }
    }
    let typed = run_command(app, "text.insert", json!({"text": text}), steps)?;
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    let place = match &d.selection.text {
        Some(t) if t.notes => json!({"notes": true}),
        Some(t) => json!({"shape": t.shape}),
        None => json!({}),
    };
    Ok(merged(json!({"slide": d.selection.slide + 1, "caret": typed.get("caret"), "newTextBox": into == TypeInto::NewBox}), place))
}

/// The current slide's empty body or content placeholder, where text belongs when nothing is selected. Only one that
/// holds text: a content placeholder that holds a picture, a chart or a table (`obj`, no text body, as a PowerPoint file
/// brings one) can't be typed in.
fn empty_body(d: &DocState) -> Option<ShapeId> {
    if d.selection.target != Target::Slides {
        return None;
    }
    let empty = |s: &&Shape| matches!(s.kind, ShapeKind::Shape) && s.text.as_ref().is_none_or(TextBody::is_empty);
    d.shapes().iter().filter(|s| !s.hidden && matches!(s.ph_type(), Some(PhType::Body | PhType::Obj))).find(empty).map(|s| s.id)
}

/// Where a new text box for Nora goes, four fifths of the slide's width: below the title, at the top of the tallest
/// strip of the slide that no shape covers; where no strip has room (or the slide is empty), across the middle. It
/// grows to fit the text.
fn new_box_rect(d: &DocState) -> Value {
    let size = d.doc.slide_size;
    let w = size.width * 0.8;
    let y = free_band(d).map_or((size.height - NEW_BOX_HEIGHT) / 2.0, |top| top + BAND_GAP);
    json!([(size.width - w) / 2.0, y, w, NEW_BOX_HEIGHT])
}

/// The top of the tallest horizontal strip, below every title, that nothing on the slide covers, when it has room for
/// a new box. `None` on an empty slide.
fn free_band(d: &DocState) -> Option<f64> {
    let height = d.doc.slide_size.height;
    let mut boxes = occupied(d);
    if boxes.is_empty() {
        return None;
    }
    let floor = boxes.iter().filter(|b| b.2).map(|b| b.1).fold(0.0, f64::max);
    boxes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut best: Option<(f64, f64)> = None;
    let mut cursor = floor;
    let mut consider = |top: f64, bottom: f64| {
        if bottom - top > best.map_or(0.0, |(_, room)| room) {
            best = Some((top, bottom - top));
        }
    };
    for (top, bottom, _) in boxes {
        if top > cursor {
            consider(cursor, top.min(height));
        }
        cursor = cursor.max(bottom);
    }
    consider(cursor, height);
    best.filter(|(_, room)| *room >= BAND_GAP + NEW_BOX_HEIGHT).map(|(top, _)| top)
}

/// The vertical extent of everything drawn on the current slide, and whether it is a title: its shapes, and the graphics
/// its layout and master draw on it (a band along the bottom, a logo), as the renderer shows them.
fn occupied(d: &DocState) -> Vec<(f64, f64, bool)> {
    let mut boxes: Vec<(f64, f64, bool)> = d
        .shapes()
        .iter()
        .filter(|s| !s.hidden)
        .map(|s| {
            let (top, bottom) = vertical_extent(deckcraft_engine::cmd::xfrm_of(&d.doc, &d.selection, s));
            (top, bottom, s.ph_type().is_some_and(PhType::is_title))
        })
        .collect();
    if d.selection.target == Target::Slides
        && let Some(slide) = d.current_slide()
        && let Some((master, layout)) = d.doc.layout(slide.layout)
    {
        let (show_layout, show_master) = deckcraft_model::resolve::show_master_shapes(slide, Some(layout));
        let drawn = master.shapes.iter().filter(|_| show_master).chain(layout.shapes.iter().filter(|_| show_layout));
        boxes.extend(drawn.filter(|s| s.ph.is_none() && !s.hidden).filter_map(|s| s.xfrm).map(|x| {
            let (top, bottom) = vertical_extent(x);
            (top, bottom, false)
        }));
    }
    boxes
}

/// The top and bottom of a box, turned by its rotation.
fn vertical_extent(x: deckcraft_model::Xfrm) -> (f64, f64) {
    let angle = x.rot.to_radians();
    let half = ((x.w * angle.sin()).abs() + (x.h * angle.cos()).abs()) / 2.0;
    let middle = x.y + x.h / 2.0;
    (middle - half, middle + half)
}

/// A slide as a picture, to look at the result (`file.render`, a base64 PNG). The page number is the slide number.
fn render_slide(app: &mut SlideApp, page: u32, scale: f64) -> Result<Value, String> {
    let count = app.session.active().ok_or(NO_PRESENTATION)?.doc.slides.len();
    let index = usize::try_from(page)
        .ok()
        .and_then(|page| page.checked_sub(1))
        .filter(|index| *index < count)
        .ok_or_else(|| format!("there is no slide {page}: the presentation has {count}"))?;
    let mut picture = app.run("file.render", json!({"slide": index, "scale": scale.clamp(0.1, 4.0)}))?;
    if let Some(map) = picture.as_object_mut() {
        map.insert("slide".into(), json!(page));
        map.insert("slides".into(), json!(count));
        map.insert("pages".into(), json!(count));
    }
    Ok(picture)
}

/// Saves the presentation with the app's own Save (`file.save`): to its file, or, for one without a file of its own,
/// through the app's Save As (the hosting page, a dialog, or a download in a browser without a host). The user said
/// yes first.
fn save(app: &mut SlideApp) -> Result<Value, String> {
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    let services = &app.services;
    let downloads = d.path.is_none() && services.pick_save.is_none() && services.host_save.is_none() && services.download.is_some();
    let said = app.status.clone();
    app.run("file.save", json!({}))?;
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    let name = name_of(d);
    if !d.is_dirty() {
        return Ok(json!({"saved": true, "name": name}));
    }
    if downloads {
        return Ok(json!({"saved": false, "downloaded": true, "name": name}));
    }
    let why = app.status.as_ref().filter(|now| said.as_ref() != Some(*now)).map(|(message, _)| format!(": {message}")).unwrap_or_default();
    Err(format!("{name} was not saved{why}"))
}

/// Takes back the last change Nora made, whole. Not when the presentation changed since: as many undo steps would then
/// take back the user's own work.
fn undo(app: &mut SlideApp) -> Result<Value, String> {
    let lang = app.nora.lang;
    let Some(change) = app.nora.changes.pop() else {
        return Err(tr(lang, "Nora has nothing left to undo.").to_string());
    };
    if change.steps == 0 {
        return Ok(json!({"undone": 0}));
    }
    let d = app.session.active().ok_or(NO_PRESENTATION)?;
    if d.uid != change.uid || !Arc::ptr_eq(&d.doc, &change.after) {
        return Err(tr(lang, "You changed the presentation after Nora did: use the editor's Undo instead.").to_string());
    }
    for _ in 0..change.steps {
        app.run("edit.undo", json!({}))?;
    }
    Ok(json!({"undone": change.steps}))
}

/// Hands one call to the host. Without a connection the call fails at once, as an unavailable server.
fn send(app: &mut SlideApp, request: HttpRequest) -> Vec<Output> {
    match &app.services.nora_http {
        Some(http) => {
            http(request);
            Vec::new()
        }
        None => app.nora.session.handle(Input::Http(HttpResponse { id: request.id, status: 503, body: String::new() })),
    }
}

/// Where the user is: the editor, the presentation's name and slide count, and the selected text.
fn context_of(app: &SlideApp) -> PageContext {
    let Some(d) = app.session.active() else { return PageContext::editor(AppKind::Deck, DocumentInfo::named("")) };
    let selected = selected_text(d);
    let selection = (!selected.trim().is_empty()).then(|| Selection { text: selected.chars().take(MAX_SELECTION_CHARS).collect() });
    PageContext::editor(AppKind::Deck, DocumentInfo { name: name_of(d), id: None, pages: u32::try_from(d.doc.slides.len()).ok(), selection })
}

/// The pane's bubbles in DeckCraft's own colours, light or dark: the person's messages on the accent, in the ink that
/// reads on it (the account button's), Nora's on the hover grey.
fn style(ui: &Ui) -> NoraStyle {
    let t = Tokens::get(ui.ctx());
    NoraStyle {
        user_fill: Some(t.accent),
        user_text: Some(crate::account::ink(&t)),
        nora_fill: Some(t.hover),
        nora_text: Some(t.text),
        ..NoraStyle::default()
    }
}

/// The words of the pane in the person's language. The pane's title row is DeckCraft's own, so the panel's title is
/// left empty.
fn labels(lang: Lang) -> NoraLabels {
    NoraLabels {
        title: String::new(),
        new_conversation: tr(lang, "New conversation").to_string(),
        undo: tr(lang, "Undo").to_string(),
        undo_hint: tr(lang, "Undo the last change Nora made to the presentation").to_string(),
        placeholder: tr(lang, "Ask Nora…").to_string(),
        send: tr(lang, "Send").to_string(),
        stop: tr(lang, "Stop").to_string(),
        working: tr(lang, "Nora is working…").to_string(),
        stopped: tr(lang, "Stopped.").to_string(),
        empty: tr(lang, "Ask Nora to write, rewrite, translate or format your presentation.").to_string(),
        confirm_heading: tr(lang, "Nora wants to:").to_string(),
        allow: tr(lang, "Allow").to_string(),
        refuse: tr(lang, "Refuse").to_string(),
        describe: describer(lang),
    }
}

/// The panel takes a plain function to describe an action, so there is one per language.
fn describer(lang: Lang) -> fn(&DocActionKind) -> String {
    match lang {
        Lang::En => |kind: &DocActionKind| describe(Lang::En, kind),
        Lang::Fr => |kind: &DocActionKind| describe(Lang::Fr, kind),
        Lang::De => |kind: &DocActionKind| describe(Lang::De, kind),
        Lang::It => |kind: &DocActionKind| describe(Lang::It, kind),
    }
}

/// The panel takes a plain function for its words on a failure of the connection, so there is one per language.
fn error_worder(lang: Lang) -> fn(&NoraError) -> String {
    match lang {
        Lang::En => |failure: &NoraError| error_words(Lang::En, failure),
        Lang::Fr => |failure: &NoraError| error_words(Lang::Fr, failure),
        Lang::De => |failure: &NoraError| error_words(Lang::De, failure),
        Lang::It => |failure: &NoraError| error_words(Lang::It, failure),
    }
}

/// A failure of the connection to Nora in the person's language (the panel shows the editor's own sentences, an action
/// it refused, as they come). The server's own words, when it gave some, are shown as they are: the server writes them
/// for the person.
fn error_words(lang: Lang, failure: &NoraError) -> String {
    match failure {
        NoraError::Timeout => tr(lang, "Nora did not answer in time.").to_string(),
        NoraError::Unreachable(_) => tr(lang, "Nora could not be reached. Check the connection and try again.").to_string(),
        NoraError::Http(401 | 403) => tr(lang, "Your session has expired. Sign in again.").to_string(),
        NoraError::Http(status) => {
            fmt(tr(lang, "The server could not answer (error {status}). Try again in a moment."), &[("status", &status.to_string())])
        }
        NoraError::Server { message, .. } => message.clone(),
        NoraError::Malformed(_) => tr(lang, "Nora's answer could not be read.").to_string(),
    }
}

/// One sentence for an action waiting for the user's yes, in the person's language.
fn describe(lang: Lang, kind: &DocActionKind) -> String {
    match kind {
        DocActionKind::Observe => tr(lang, "Read the presentation").to_string(),
        DocActionKind::Execute { command, .. } => fmt(tr(lang, "Run {command}"), &[("command", command)]),
        DocActionKind::Batch { commands } => match commands.as_slice() {
            [only] => fmt(tr(lang, "Run {command}"), &[("command", &only.command)]),
            _ => fmt(tr(lang, "Run {count} commands"), &[("count", &commands.len().to_string())]),
        },
        DocActionKind::SelectText { text, .. } => fmt(tr(lang, "Select \"{text}\""), &[("text", &shorten(text))]),
        DocActionKind::TypeText { text, .. } => fmt(tr(lang, "Type \"{text}\""), &[("text", &shorten(text))]),
        DocActionKind::RenderPage { page, .. } => fmt(tr(lang, "Look at slide {slide}"), &[("slide", &page.to_string())]),
        DocActionKind::Save => tr(lang, "Save the presentation").to_string(),
        DocActionKind::Undo => tr(lang, "Undo the last change").to_string(),
    }
}

/// The start of a text, for a sentence that quotes it.
fn shorten(text: &str) -> String {
    let mut short: String = text.chars().take(40).collect();
    if text.chars().count() > 40 {
        short.push('…');
    }
    short
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Mutex;

    use deckcraft_engine::Session;
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::{NoraInbox, Services, UiState};

    type Outbox = Arc<Mutex<Vec<HttpRequest>>>;
    type Harness = egui_kittest::Harness<'static, SlideApp>;

    fn app(services: Services) -> SlideApp {
        let mut app = SlideApp::new(Session::with_new(), services);
        // The speaker notes under the slide are a text box too: hidden, so that the pane's is the only one.
        app.ui.notes = false;
        app
    }

    /// A host that records the calls and lets the test answer them.
    fn connected() -> (Services, Outbox, NoraInbox) {
        let outbox: Outbox = Arc::default();
        let inbox: NoraInbox = Arc::default();
        let sent = outbox.clone();
        let services = Services {
            nora_http: Some(Box::new(move |request| sent.lock().unwrap_or_else(|e| e.into_inner()).push(request))),
            nora_inbox: Some(inbox.clone()),
            ..Services::default()
        };
        (services, outbox, inbox)
    }

    fn harness(app: SlideApp) -> Harness {
        let mut h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut SlideApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
        // The app keeps asking for frames, so frames are stepped rather than run to rest.
        h.run_steps(4);
        h
    }

    fn answer(inbox: &NoraInbox, request: &HttpRequest, message: Value) {
        let body = json!({ "message": message }).to_string();
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push(HttpResponse { id: request.id, status: 200, body });
    }

    fn take(outbox: &Outbox) -> Vec<HttpRequest> {
        std::mem::take(&mut *outbox.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Sends `message` from the pane, gives the conversation an id, and returns the reads that follow.
    fn start(h: &mut Harness, outbox: &Outbox, inbox: &NoraInbox, message: &str) -> Vec<HttpRequest> {
        h.state_mut().nora.panel.set_draft(message);
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        let first = take(outbox);
        answer(inbox, &first[0], json!({"thread_id": "t1"}));
        h.run_steps(6);
        take(outbox)
    }

    /// The presentation the tests work on: slide 1 "Welcome" over "Hello big world", slide 2 "Plan" with two points;
    /// slide 1 current, nothing selected.
    fn deck() -> SlideApp {
        let mut a = app(Services::default());
        let (title, subtitle) = (shape_id(&a, 0, 0), shape_id(&a, 0, 1));
        a.run("text.set", json!({"id": title, "text": "Welcome"})).unwrap();
        a.run("text.set", json!({"id": subtitle, "text": "Hello big world"})).unwrap();
        a.run("slide.new", json!({"title": "Plan", "body": "First point\nSecond point"})).unwrap();
        a.run("slide.go", json!({"index": 0})).unwrap();
        deselect(&mut a);
        a
    }

    fn deselect(a: &mut SlideApp) {
        a.run("text.exit", json!({})).unwrap();
        a.run("edit.deselect", json!({})).unwrap();
    }

    fn shape_id(a: &SlideApp, slide: usize, at: usize) -> u32 {
        a.session.active().unwrap().doc.slides[slide].shapes[at].id.0
    }

    fn shape_text(a: &SlideApp, slide: usize, at: usize) -> String {
        a.session.active().unwrap().doc.slides[slide].shapes[at].text.as_ref().map(TextBody::text).unwrap_or_default()
    }

    fn slides(a: &SlideApp) -> usize {
        a.session.active().unwrap().doc.slides.len()
    }

    /// The text body being edited (the shape Nora just typed in).
    fn edited(a: &SlideApp) -> TextBody {
        let d = a.session.active().unwrap();
        let t = d.selection.text.clone().expect("a text is being edited");
        d.shape(t.shape).and_then(|s| s.text.clone()).unwrap_or_default()
    }

    fn run(a: &mut SlideApp, kind: DocActionKind) -> Result<Value, String> {
        run_document(a, &kind)
    }

    fn type_text(text: &str, paragraphs: bool) -> DocActionKind {
        DocActionKind::TypeText { text: text.into(), paragraphs }
    }

    fn select(text: &str, occurrence: u32) -> DocActionKind {
        DocActionKind::SelectText { text: text.into(), occurrence }
    }

    fn execute(command: &str, params: Value) -> DocActionKind {
        DocActionKind::Execute { command: command.into(), params }
    }

    fn call(command: &str, params: Value) -> CommandCall {
        CommandCall { command: command.into(), params }
    }

    /// The Nora pane is a UI command: off by default, toggled without a value, kept between runs.
    #[test]
    fn nora_is_a_ui_command_kept_between_runs() {
        let mut a = app(Services::default());
        assert!(!a.ui.nora_pane, "off by default");
        assert_eq!(a.run("ui.nora", json!({})).unwrap(), json!({"nora": true}), "without a value it toggles");
        assert_eq!(a.run("ui.nora", json!({})).unwrap(), json!({"nora": false}));
        assert_eq!(a.run("ui.nora", json!({"on": true})).unwrap(), json!({"nora": true}));
        assert_eq!(a.run("ui.nora", json!({"value": false})).unwrap(), json!({"nora": false}), "WordCraft's name for it works too");
        a.run("ui.nora", json!({"on": true})).unwrap();
        for junk in [json!({"on": "yes"}), json!({"on": 1}), json!({"on": null}), json!({"value": [true]})] {
            assert!(a.run("ui.nora", junk).is_err());
        }
        assert!(a.ui.nora_pane, "a rejected value keeps the setting");
        let saved = serde_json::to_string(&a.ui).unwrap();
        assert!(saved.contains(r#""noraPane":true"#), "{saved}");
        assert!(serde_json::from_str::<UiState>(&saved).unwrap().nora_pane);
        // Settings saved before the pane existed leave it off.
        assert!(!serde_json::from_str::<UiState>(r#"{"tab":"Insert"}"#).unwrap().nora_pane);
        // Listed with the other UI commands (the command palette, agents' `engine.commands`).
        let listed = crate::control::all_commands(&a);
        assert!(listed.as_array().unwrap().iter().any(|c| c["id"] == "ui.nora" && c["label"] == "Nora"));
    }

    /// Preferences › Assistant is the `ui.nora` setting, in the person's language.
    #[test]
    fn the_preferences_switch_shows_and_hides_the_pane() {
        let mut a = app(Services::default());
        a.run("app.preferences", json!({})).unwrap();
        let mut h = harness(a);
        assert!(h.query_by_label("Assistant").is_some(), "the section is there");
        h.get_by_label("Show the Nora assistant pane (experimental)").click();
        h.run_steps(4);
        assert!(h.state().ui.nora_pane, "the checkbox turns it on");
        h.get_by_label("Show the Nora assistant pane (experimental)").click();
        h.run_steps(4);
        assert!(!h.state().ui.nora_pane, "and off again");

        let mut a = app(Services::default());
        a.nora.lang = Lang::Fr;
        a.run("app.preferences", json!({})).unwrap();
        let h = harness(a);
        assert!(h.query_by_label("Afficher le volet de l'assistant Nora (expérimental)").is_some());
    }

    /// Nothing is drawn while the pane is off, and without a connection the pane says why it can't be used instead of
    /// offering a text box.
    #[test]
    fn without_a_connection_the_pane_says_nora_isnt_available() {
        let mut h = harness(app(Services::default()));
        assert!(h.query_by_label("Nora isn't available in this version yet").is_none(), "off by default");
        h.state_mut().run("ui.nora", json!({"on": true})).unwrap();
        h.run_steps(4);
        assert!(h.query_all_by_label("Nora").next().is_some(), "the pane's title");
        assert!(h.query_by_label("Nora isn't available in this version yet").is_some());
        assert!(h.query_by_role(Role::MultilineTextInput).is_none(), "no text box to lose a message in");
        // The title row's close button is a painted icon that runs this very command.
        h.state_mut().run("ui.nora", json!({"on": false})).unwrap();
        h.run_steps(4);
        assert!(h.query_by_label("Nora isn't available in this version yet").is_none(), "closed, it draws nothing");
    }

    /// The words of the pane follow the person's language.
    #[test]
    fn the_pane_speaks_the_persons_language() {
        for (lang, offline, new_conversation) in [
            (Lang::Fr, "Nora n'est pas encore disponible dans cette version", "Nouvelle conversation"),
            (Lang::De, "Nora ist in dieser Version noch nicht verfügbar", "Neues Gespräch"),
            (Lang::It, "Nora non è ancora disponibile in questa versione", "Nuova conversazione"),
        ] {
            let mut a = app(Services::default());
            a.nora.lang = lang;
            a.run("ui.nora", json!({"on": true})).unwrap();
            let h = harness(a);
            assert!(h.query_by_label(offline).is_some(), "{lang:?}");
            assert!(h.query_by_label(new_conversation).is_some(), "{lang:?}");
            assert!(h.query_by_label("New conversation").is_none(), "{lang:?}");
        }
        let mut a = app(Services::default());
        a.nora.lang = Lang::Fr;
        a.run("ui.nora", json!({"on": true})).unwrap();
        let h = harness(a);
        assert!(h.query_by_label("Demandez à Nora d'écrire, de réécrire, de traduire ou de mettre en forme votre présentation.").is_some());
    }

    /// A message typed in the pane leaves as the first call of the conversation, carrying where the person is; the
    /// host's answers show up in the pane; the presentation is left alone.
    #[test]
    fn a_connected_pane_sends_the_message_with_its_context_and_shows_the_reply() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"on": true})).unwrap();
        let mut h = harness(a);
        let before = Arc::clone(&h.state().session.active().unwrap().doc);
        assert!(h.query_by_label("Nora isn't available in this version yet").is_none());

        h.get_by_role(Role::MultilineTextInput).focus();
        h.run_steps(2);
        h.get_by_role(Role::MultilineTextInput).type_text("Make the title friendlier");
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(4);

        let calls = take(&outbox);
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].path.ends_with("nora.api.chat.start_or_resume_thread"), "{}", calls[0].path);
        assert_eq!(calls[0].body["message"], "Make the title friendlier");
        let context: Value = serde_json::from_str(calls[0].body["context"].as_str().unwrap_or_default()).unwrap();
        assert_eq!(context["surface"], "editor");
        assert_eq!(context["app"], "deck");
        assert_eq!(context["document"]["name"], "Presentation1");
        assert_eq!(context["document"]["pages"], 1, "the slide count");
        assert!(h.query_by_label("Make the title friendlier").is_some(), "the person's own message is in the transcript");
        assert!(Arc::ptr_eq(&before, &h.state().session.active().unwrap().doc), "typing in the pane never edits the presentation");

        // The server gives the conversation an id; the pane then reads the replies and the actions.
        answer(&inbox, &calls[0], json!({"thread_id": "t1", "sent_at": 1000.0}));
        h.run_steps(6);
        let polls = take(&outbox);
        let reads = polls.iter().find(|r| r.path.ends_with("get_messages")).expect("the replies are read");
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        // As the real server stamps them: an older reply is not Nora's answer to this message.
        let replies = json!([
            {"role": "assistant", "text": "An answer from an earlier conversation.", "ts": 900.0},
            {"role": "assistant", "text": "Done, the title is friendlier.", "ts": 1001.0}
        ]);
        answer(&inbox, reads, json!({"messages": replies}));
        answer(&inbox, actions, json!({"actions": []}));
        h.run_steps(4);
        assert!(h.query_by_label("Done, the title is friendlier.").is_some(), "Nora's reply is shown");
        assert!(h.query_by_label("An answer from an earlier conversation.").is_none(), "a reply older than the send is not");
    }

    /// What Nora asks for in the presentation is played, and she gets the result by callback.
    #[test]
    fn a_document_action_runs_and_nora_gets_the_observation() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"on": true})).unwrap();
        let mut h = harness(a);
        let polls = start(&mut h, &outbox, &inbox, "Write a greeting");
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        answer(&inbox, actions, json!({"actions": [{"action": "doc_type_text", "text": "Bonjour tout le monde", "callback_id": "c1"}]}));
        h.run_steps(4);
        let typed = h.state().session.active().unwrap().doc.slides[0]
            .shapes
            .iter()
            .any(|s| s.text.as_ref().is_some_and(|t| t.text() == "Bonjour tout le monde"));
        assert!(typed, "the text was typed in a new text box on the slide");
        let told = take(&outbox);
        let observation = told.iter().find(|r| r.path.ends_with("receive_observation")).expect("Nora gets an answer");
        assert_eq!(observation.body["callback_id"], "c1");
        assert_eq!(observation.body["observation"]["ok"], true);
        assert_eq!(observation.body["observation"]["result"]["newTextBox"], true);
    }

    /// A command the policy refuses never reaches the presentation nor the interface, and Nora is told why.
    #[test]
    fn a_refused_command_never_runs() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"on": true})).unwrap();
        let mut h = harness(a);
        let polls = start(&mut h, &outbox, &inbox, "Switch to dark and close your pane");
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        let asked = json!({"actions": [
            {"action": "doc_execute", "command": "view.dark", "params": {"on": true}, "callback_id": "c2"},
            {"action": "doc_execute", "command": "ui.nora", "params": {"on": false}, "callback_id": "c3"}
        ]});
        answer(&inbox, actions, asked);
        h.run_steps(4);
        let told = take(&outbox);
        for callback in ["c2", "c3"] {
            let observation =
                told.iter().find(|r| r.path.ends_with("receive_observation") && r.body["callback_id"] == callback).expect("Nora gets an answer");
            assert_eq!(observation.body["observation"]["ok"], false, "{callback}");
            assert!(observation.body["observation"]["error"].as_str().unwrap_or_default().contains("refused"), "{callback}");
        }
        assert_eq!(h.state().ui.brightness, theme::Brightness::Light, "the person's appearance is theirs");
        assert!(h.state().ui.nora_pane, "and so is the pane");
    }

    #[test]
    fn nora_reads_every_slide_in_order_with_the_current_one_marked() {
        let mut a = deck();
        let seen = run(&mut a, DocActionKind::Observe).unwrap();
        assert_eq!(seen["text"], "Slide 1 (current): Welcome\nHello big world\n\nSlide 2: Plan\nFirst point\nSecond point");
        assert_eq!(seen["slides"], 2);
        assert_eq!(seen["pages"], 2, "the slide count, under the name every editor uses");
        assert_eq!(seen["currentSlide"], 1);
        assert_eq!(seen["words"], 9);
        assert_eq!(seen["textTruncated"], false);
        assert_eq!(seen["name"], "Presentation1");
        assert_eq!(seen["selection"]["slide"], 1);
        assert_eq!(seen["selection"]["text"], "");
        // Another current slide, its notes, and a hidden slide.
        a.run("slide.notes", json!({"index": 1, "text": "Smile"})).unwrap();
        a.run("slide.hide", json!({"index": 1, "hidden": true})).unwrap();
        a.run("slide.go", json!({"index": 1})).unwrap();
        let seen = run(&mut a, DocActionKind::Observe).unwrap();
        assert_eq!(seen["text"], "Slide 1: Welcome\nHello big world\n\nSlide 2 (current, hidden): Plan\nFirst point\nSecond point\nNotes: Smile");
        assert_eq!(seen["currentSlide"], 2);
        assert_eq!(seen["words"], 9, "notes are not slide text");
    }

    #[test]
    fn what_nora_reads_is_cut_at_a_length_that_fits_her() {
        let mut a = deck();
        let subtitle = shape_id(&a, 0, 1);
        a.run("text.set", json!({"id": subtitle, "text": "é".repeat(MAX_OBSERVED_CHARS + 10)})).unwrap();
        let seen = run(&mut a, DocActionKind::Observe).unwrap();
        assert_eq!(seen["text"].as_str().map(|text| text.chars().count()), Some(MAX_OBSERVED_CHARS));
        assert_eq!(seen["textTruncated"], true);
    }

    #[test]
    fn selecting_then_formatting_changes_the_selection_only() {
        let mut a = deck();
        let found = run(&mut a, select("big", 1)).unwrap();
        assert_eq!((found["slide"].clone(), found["anchor"].clone(), found["caret"].clone()), (json!(1), json!([0, 6]), json!([0, 9])));
        assert_eq!(found["shape"], shape_id(&a, 0, 1));
        run(&mut a, execute("format.bold", json!({}))).unwrap();
        assert_eq!(shape_text(&a, 0, 1), "Hello big world", "the words are the same");
        let body = a.session.active().unwrap().doc.slides[0].shapes[1].text.clone().unwrap();
        let bold: String = body.paragraphs[0].runs.iter().filter(|r| r.props.bold == Some(true)).map(|r| r.text.as_str()).collect();
        assert_eq!(bold, "big", "only the selection is bold");
        assert_eq!(run(&mut a, DocActionKind::Observe).unwrap()["selection"]["text"], "big");
    }

    #[test]
    fn select_text_finds_the_nth_occurrence_on_the_current_slide() {
        let mut a = deck();
        let title = shape_id(&a, 0, 0);
        a.run("text.set", json!({"id": title, "text": "one two one"})).unwrap();
        let second = run(&mut a, select("one", 2)).unwrap();
        assert_eq!((second["shape"].clone(), second["anchor"].clone(), second["caret"].clone()), (json!(title), json!([0, 8]), json!([0, 11])));
        let error = run(&mut a, select("Plan", 1)).unwrap_err();
        assert!(error.contains("not on slide 1"), "another slide's text is not this slide's: {error}");
        let error = run(&mut a, select("one", 3)).unwrap_err();
        assert!(error.contains("2 time(s), not 3"), "{error}");
        assert_eq!(
            a.session.active().unwrap().selection.text.as_ref().map(|t| (t.anchor, t.caret)),
            Some(((0, 8), (0, 11))),
            "a failed search keeps the selection"
        );
        // The slide's notes are on the slide too, after its shapes.
        a.run("slide.notes", json!({"index": 0, "text": "remember the one"})).unwrap();
        let note = run(&mut a, select("remember", 1)).unwrap();
        assert_eq!(note["notes"], true);
        assert!(a.session.active().unwrap().selection.text.as_ref().is_some_and(|t| t.notes && t.anchor == (0, 0) && t.caret == (0, 8)));
    }

    #[test]
    fn typing_goes_into_the_text_being_edited_else_the_selected_shape_else_a_new_text_box() {
        let mut a = deck();
        // The text being edited: its selection is replaced.
        run(&mut a, select("big", 1)).unwrap();
        run(&mut a, type_text("small", false)).unwrap();
        assert_eq!(shape_text(&a, 0, 1), "Hello small world");
        // One selected shape: its text is replaced, as typing on it does.
        deselect(&mut a);
        let title = shape_id(&a, 0, 0);
        a.run("edit.select", json!({"ids": [title]})).unwrap();
        let typed = run(&mut a, type_text("Bienvenue", false)).unwrap();
        assert_eq!(shape_text(&a, 0, 0), "Bienvenue");
        assert_eq!((typed["shape"].clone(), typed["newTextBox"].clone(), typed["slide"].clone()), (json!(title), json!(false), json!(1)));
        // Nothing selected: a new text box on the current slide.
        deselect(&mut a);
        deselect(&mut a);
        let shapes = a.session.active().unwrap().doc.slides[0].shapes.len();
        let typed = run(&mut a, type_text("A word from Nora", false)).unwrap();
        assert_eq!(typed["newTextBox"], true);
        let slide = &a.session.active().unwrap().doc.slides[0];
        assert_eq!(slide.shapes.len(), shapes + 1);
        assert_eq!(slide.shapes.last().and_then(|s| s.text.as_ref()).map(TextBody::text).as_deref(), Some("A word from Nora"));
        // A slide-less presentation has nowhere to type: an error, and nothing changes.
        a.run("file.new", json!({"blank": true})).unwrap();
        let error = run(&mut a, type_text("Hi", false)).unwrap_err();
        assert!(error.contains("no slide"), "{error}");
        assert_eq!(slides(&a), 0);
    }

    #[test]
    fn with_paragraphs_each_line_is_a_paragraph_and_without_a_line_of_one() {
        let mut a = deck();
        run(&mut a, type_text("one\ntwo", true)).unwrap();
        assert_eq!(edited(&a).paragraphs.len(), 2);
        deselect(&mut a);
        deselect(&mut a);
        run(&mut a, type_text("three\r\nfour", false)).unwrap();
        let body = edited(&a);
        assert_eq!(body.paragraphs.len(), 1, "one paragraph");
        assert_eq!(plain(&body), "three\nfour", "two lines");
    }

    #[test]
    fn nora_types_her_words_as_she_wrote_them() {
        let mut a = deck();
        assert!(a.session.prefs.autocorrect && a.session.prefs.smart_quotes);
        run(&mut a, type_text("teh (c) ", false)).unwrap();
        assert_eq!(plain(&edited(&a)), "teh (c) ", "no AutoCorrect on Nora's words");
        assert!(a.session.prefs.autocorrect && a.session.prefs.smart_quotes, "the person's settings are back");
        // The person's own typing is corrected as before.
        a.run("text.insert", json!({"text": " teh "})).unwrap();
        assert_eq!(plain(&edited(&a)), "teh (c)  the ");
    }

    #[test]
    fn undo_takes_back_a_whole_batch_and_only_what_nora_did() {
        let mut a = deck();
        let title = shape_id(&a, 0, 0);
        a.run("text.set", json!({"id": title, "text": "Mine"})).unwrap();
        let commands =
            vec![call("slide.new", json!({"title": "Three"})), call("slide.new", json!({"title": "Four"})), call("slide.go", json!({"index": 0}))];
        let results = run(&mut a, DocActionKind::Batch { commands }).unwrap();
        assert_eq!(results.as_array().map(Vec::len), Some(3));
        assert_eq!(slides(&a), 4);
        let undone = run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(undone["undone"], 2, "two steps: the two new slides; going to a slide is none");
        assert_eq!(slides(&a), 2, "the whole batch is gone");
        assert_eq!(shape_text(&a, 0, 0), "Mine", "and the person's own change stays");
        assert!(run(&mut a, DocActionKind::Undo).is_err(), "nothing left of Nora's to undo");
    }

    #[test]
    fn undo_leaves_a_presentation_changed_since_alone() {
        let mut a = deck();
        a.nora.lang = Lang::Fr;
        run(&mut a, execute("slide.new", json!({"title": "Three"}))).unwrap();
        // The person changes the presentation after Nora: undoing a step now would take back theirs.
        a.run("slide.new", json!({"title": "Mine"})).unwrap();
        let before = Arc::clone(&a.session.active().unwrap().doc);
        let error = run(&mut a, DocActionKind::Undo).unwrap_err();
        assert_eq!(error, "Vous avez modifié la présentation après Nora : utilisez plutôt la fonction Annuler de l'éditeur.");
        assert!(Arc::ptr_eq(&a.session.active().unwrap().doc, &before), "nothing is undone");
        assert_eq!(slides(&a), 4);
    }

    #[test]
    fn undo_counts_nora_s_steps_when_the_history_is_full() {
        let mut a = deck();
        a.session.active_mut().unwrap().history.limit = 3;
        for n in 0..5 {
            a.run("slide.new", json!({"title": format!("Mine {n}")})).unwrap();
        }
        assert_eq!(a.session.active().unwrap().history.undo.len(), 3, "full: each new step drops the oldest");
        run(&mut a, execute("slide.new", json!({"title": "Nora's"}))).unwrap();
        let count = slides(&a);
        assert_eq!(run(&mut a, DocActionKind::Undo).unwrap()["undone"], 1);
        assert_eq!(slides(&a), count - 1);
    }

    #[test]
    fn a_failing_batch_leaves_the_presentation_as_it_was() {
        let mut a = deck();
        // A step to redo, so that the redo history is there to keep as well.
        a.run("slide.new", json!({"title": "Undone"})).unwrap();
        a.run("edit.undo", json!({})).unwrap();
        let (doc, undo, redo, slide) = {
            let d = a.session.active().unwrap();
            (Arc::clone(&d.doc), d.history.undo.len(), d.history.redo.len(), d.selection.slide)
        };
        assert_eq!(redo, 1);
        let remembered = a.nora.changes.len();
        let commands = vec![call("slide.new", json!({"title": "Three"})), call("slide.go", json!({"index": 2})), call("no.such.command", json!({}))];
        let error = run(&mut a, DocActionKind::Batch { commands }).unwrap_err();
        assert!(error.contains("no.such.command"), "{error}");
        let d = a.session.active().unwrap();
        assert!(Arc::ptr_eq(&d.doc, &doc), "the presentation from before");
        assert_eq!((d.history.undo.len(), d.history.redo.len(), d.selection.slide), (undo, redo, slide), "its history and its current slide too");
        assert_eq!(a.nora.changes.len(), remembered, "and nothing new to undo");
        a.run("edit.redo", json!({})).unwrap();
        assert_eq!(slides(&a), 3, "the person's redo still works");
    }

    #[test]
    fn no_command_of_nora_reaches_a_file_of_the_computer() {
        let mut a = deck();
        for params in [json!({"path": "/etc/hosts"}), json!({"options": {"FileName": "x"}}), json!({"list": [{"directory": "/"}]})] {
            let error = run(&mut a, execute("insert.picture", params)).unwrap_err();
            assert!(error.contains("refused"), "{error}");
        }
        let batch = vec![call("slide.new", json!({})), call("insert.video", json!({"path": "x.mp4"}))];
        assert!(run(&mut a, DocActionKind::Batch { commands: batch }).is_err());
        assert_eq!(slides(&a), 2, "the batch's first command was taken back");
    }

    #[test]
    fn the_policy_keeps_the_interface_and_the_files_to_the_user() {
        use craft_nora::Decision;
        let policy = policy();
        for allowed in [
            "format.bold",
            "slide.new",
            "text.insert",
            "text.delete",
            "view.slideMaster",
            "show.setup",
            "document.inspect",
            "media.pause",
            "media.info",
        ] {
            assert_eq!(policy.check(allowed), Decision::Allow, "{allowed}");
        }
        for denied in [
            "ui.nora",
            "ui.anything",
            "view.dark",
            "view.zoom",
            "view.pane",
            "view.sorter",
            "show.start",
            "show.fromStart",
            "window.next",
            "app.quit",
            "file.save",
            "file.export",
            // The person's own history, clipboard and media playback.
            "edit.undo",
            "edit.redo",
            "edit.copy",
            "edit.cut",
            "edit.paste",
            "edit.pasteText",
            "media.play",
            "media.seek",
            "media.toggle",
            "media.posterFrame",
        ] {
            assert_eq!(policy.check(denied), Decision::Deny, "{denied}");
        }
        for (id, ..) in crate::UI_COMMANDS {
            assert_eq!(policy.check(id), Decision::Deny, "{id} is the interface's");
        }
        // No other editing command of the engine is caught by a prefix: only files, windows, the slide show, the person's
        // history and clipboard (Paste Special too: `edit.paste` covers it), and media playback are refused, and the Notes
        // Master view, which the prefix of the interface's `view.notes` (the Notes pane) covers: the Slide and Handout
        // Masters stay Nora's.
        let theirs = [
            "show.fromStart",
            "show.fromCurrent",
            "edit.undo",
            "edit.redo",
            "edit.copy",
            "edit.cut",
            "edit.paste",
            "edit.pasteText",
            "edit.pasteSpecial",
            "view.notesMaster",
            "media.play",
            "media.seek",
            "media.toggle",
            "media.posterFrame",
        ];
        for spec in deckcraft_engine::command_specs() {
            let refused = spec.id.starts_with("file.") || spec.id.starts_with("window.") || theirs.contains(&spec.id);
            assert_eq!(policy.check(spec.id) == Decision::Deny, refused, "{}", spec.id);
        }
    }

    /// Nora can't take back the person's work: a batch that undoes, or a bare undo, is refused before anything runs (the
    /// pane's Undo would otherwise take back the person's previous edit as Nora's).
    #[test]
    fn nora_never_undoes_the_person_s_work() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"on": true})).unwrap();
        a.run("slide.new", json!({"title": "Mine"})).unwrap();
        let mut h = harness(a);
        let before = Arc::clone(&h.state().session.active().unwrap().doc);
        let polls = start(&mut h, &outbox, &inbox, "Tidy up");
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        let asked = json!({"actions": [
            {"action": "doc_batch", "commands": [{"command": "slide.new", "params": {"title": "Nora's"}}, {"command": "edit.undo"}], "callback_id": "c1"},
            {"action": "doc_execute", "command": "edit.undo", "callback_id": "c2"},
            {"action": "doc_execute", "command": "edit.redo", "callback_id": "c3"}
        ]});
        answer(&inbox, actions, asked);
        h.run_steps(4);
        let told = take(&outbox);
        for callback in ["c1", "c2", "c3"] {
            let observation =
                told.iter().find(|r| r.path.ends_with("receive_observation") && r.body["callback_id"] == callback).expect("Nora gets an answer");
            assert_eq!(observation.body["observation"]["ok"], false, "{callback}");
            assert!(observation.body["observation"]["error"].as_str().unwrap_or_default().contains("refused"), "{callback}");
        }
        assert!(Arc::ptr_eq(&before, &h.state().session.active().unwrap().doc), "nothing ran: the person's slide is still there");
    }

    /// `dir` is a direction (Delete Text, Text Direction, Distribute, a shadow's), not a folder: the engine reads only
    /// `path`.
    #[test]
    fn a_dir_parameter_is_a_direction_not_a_file() {
        let mut a = deck();
        run(&mut a, select("world", 1)).unwrap();
        run(&mut a, execute("text.move", json!({"to": "end"}))).unwrap();
        run(&mut a, execute("text.delete", json!({"dir": "wordBackward"}))).unwrap();
        assert_eq!(shape_text(&a, 0, 1), "Hello big ");
        deselect(&mut a);
        a.run("edit.select", json!({"ids": [shape_id(&a, 0, 1)]})).unwrap();
        run(&mut a, execute("format.direction", json!({"dir": "rotate90"}))).unwrap();
        run(&mut a, execute("shape.effects", json!({"shadow": {"color": "#000000", "blur": 4, "dist": 3, "dir": 45}}))).unwrap();
    }

    /// A drag of a slide in the thumbnails or the slide sorter moves slides by their index: Nora's actions wait for it
    /// to end, as they wait for a drag on the slide.
    #[test]
    fn nora_waits_while_the_person_drags_a_slide_thumbnail() {
        let mut a = deck();
        a.thumb_drag = Some((0, 1));
        let before = Arc::clone(&a.session.active().unwrap().doc);
        let ctx = egui::Context::default();
        perform(&mut a, vec![Output::Document(DocumentCall { id: 1, kind: execute("slide.delete", json!({"index": 0})) })], &ctx);
        assert!(Arc::ptr_eq(&a.session.active().unwrap().doc, &before), "no slide moves under the person's drag");
        assert!(released(&mut a).is_empty(), "still dragging");
        a.thumb_drag = None;
        let outputs = released(&mut a);
        perform(&mut a, outputs, &ctx);
        assert_eq!(slides(&a), 1, "played once the drag ended");
    }

    /// A section dragged in the slide sorter, or an object's row in the Selection pane, moves things by their index
    /// too: Nora's actions wait for those drags as well.
    #[test]
    fn nora_waits_while_the_person_drags_a_section_or_a_selection_pane_row() {
        let ctx = egui::Context::default();
        for section in [true, false] {
            let mut a = deck();
            if section {
                a.sorter.drag = Some((0, 1));
            } else {
                a.selpane.drag = Some((1, None));
            }
            let before = Arc::clone(&a.session.active().unwrap().doc);
            perform(&mut a, vec![Output::Document(DocumentCall { id: 1, kind: execute("slide.delete", json!({"index": 0})) })], &ctx);
            assert!(Arc::ptr_eq(&a.session.active().unwrap().doc, &before), "nothing moves under the person's drag (section {section})");
            assert!(released(&mut a).is_empty(), "still dragging (section {section})");
            a.sorter.drag = None;
            a.selpane.drag = None;
            let outputs = released(&mut a);
            perform(&mut a, outputs, &ctx);
            assert_eq!(slides(&a), 1, "played once the drag ended (section {section})");
        }
    }

    /// A tool's drag on the slide (drawing a shape here) and a guide being dragged hold Nora's actions too, until the
    /// pointer lets go.
    #[test]
    fn nora_waits_while_a_tool_draws_or_a_guide_moves() {
        let ctx = egui::Context::default();
        let press = |a: &mut SlideApp, kind: deckcraft_engine::PointerKind, x: f64, y: f64| {
            let ev = deckcraft_engine::PointerEvent { kind, x, y, mods: Default::default(), tol: 4.0, pressure: None };
            a.session.pointer(ev).unwrap();
        };
        for guide in [false, true] {
            let mut a = deck();
            if guide {
                a.session.tool.guide_drag = Some((0, 100.0));
            } else {
                a.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: "rect".into() });
                press(&mut a, deckcraft_engine::PointerKind::Down, 500.0, 400.0);
                assert!(a.session.tool.dragging(), "the rectangle is being drawn");
            }
            let before = Arc::clone(&a.session.active().unwrap().doc);
            perform(&mut a, vec![Output::Document(DocumentCall { id: 1, kind: execute("slide.delete", json!({"index": 0})) })], &ctx);
            assert!(Arc::ptr_eq(&a.session.active().unwrap().doc, &before), "nothing changes under the gesture (guide {guide})");
            assert!(released(&mut a).is_empty(), "still dragging (guide {guide})");
            if guide {
                a.session.tool.guide_drag = None;
            } else {
                press(&mut a, deckcraft_engine::PointerKind::Up, 600.0, 450.0);
                assert!(!a.session.tool.dragging());
            }
            let outputs = released(&mut a);
            perform(&mut a, outputs, &ctx);
            assert_eq!(slides(&a), 1, "played once the gesture ended (guide {guide})");
        }
    }

    /// Actions that arrive in the frame the pointer lets go play after the ones that waited for it, in their order.
    #[test]
    fn actions_arriving_as_the_pointer_lets_go_play_after_those_that_waited() {
        let (services, _outbox, inbox) = connected();
        let mut a = app(services);
        // A conversation whose action queue is being read.
        let sent = a.nora.session.handle(Input::Send("Go".into()));
        let first = sent.iter().find_map(|o| if let Output::Http(r) = o { Some(r.clone()) } else { None }).expect("the message is sent");
        let body = json!({"message": {"thread_id": "t1"}}).to_string();
        a.nora.session.handle(Input::Http(HttpResponse { id: first.id, status: 200, body }));
        let polls = a.nora.session.handle(Input::Tick { now_ms: 10 });
        let actions = polls
            .iter()
            .find_map(|o| if let Output::Http(r) = o { r.path.ends_with("get_browser_actions").then(|| r.clone()) } else { None })
            .expect("the actions are read");
        // An older action waited for the pointer; a newer one arrives in the frame the pointer lets go.
        a.nora.waiting.push(DocumentCall { id: 900, kind: execute("slide.new", json!({"title": "First"})) });
        answer(&inbox, &actions, json!({"actions": [{"action": "doc_execute", "command": "slide.new", "params": {"title": "Second"}}]}));
        let outputs = incoming(&mut a);
        perform(&mut a, outputs, &egui::Context::default());
        let titles: Vec<String> = a.session.active().unwrap().doc.slides.iter().map(|s| s.title()).collect();
        assert_eq!(titles, ["", "First", "Second"], "in the order Nora asked");
    }

    /// The box of a text with no place to go: below the title, in the tallest strip of the slide no shape covers.
    #[test]
    fn a_new_text_box_goes_in_the_free_band_below_the_title() {
        let mut a = deck();
        // A band along the bottom, as many title slides have.
        let size = a.session.active().unwrap().doc.slide_size;
        a.run("shape.insert", json!({"preset": "rect", "rect": [0.0, size.height - 60.0, size.width, 60.0]})).unwrap();
        deselect(&mut a);
        deselect(&mut a);
        let boxes = |a: &SlideApp| -> Vec<(f64, f64, bool)> {
            let d = a.session.active().unwrap();
            d.shapes()
                .iter()
                .map(|s| {
                    let x = deckcraft_engine::cmd::xfrm_of(&d.doc, &d.selection, s);
                    (x.y, x.y + x.h, s.ph_type().is_some_and(PhType::is_title))
                })
                .collect()
        };
        let before = boxes(&a);
        let typed = run(&mut a, type_text("A word from Nora", false)).unwrap();
        assert_eq!(typed["newTextBox"], true);
        let after = boxes(&a);
        let (top, bottom, _) = *after.last().unwrap();
        let title_bottom = before.iter().filter(|b| b.2).map(|b| b.1).fold(0.0, f64::max);
        assert!(top >= title_bottom, "never over the title: the box starts at {top}, the title ends at {title_bottom}");
        for (other_top, other_bottom, _) in &before {
            assert!(bottom <= *other_top || top >= *other_bottom, "the box {top}..{bottom} covers a shape at {other_top}..{other_bottom}");
        }
    }

    /// With no selection, Nora's text goes into the slide's empty body placeholder when it has one.
    #[test]
    fn typing_fills_the_slide_s_empty_content_placeholder() {
        let mut a = deck();
        a.run("slide.new", json!({"title": "Agenda"})).unwrap();
        deselect(&mut a);
        let shapes = a.session.active().unwrap().shapes().len();
        let typed = run(&mut a, type_text("One\nTwo", true)).unwrap();
        assert_eq!(typed["newTextBox"], false);
        let d = a.session.active().unwrap();
        assert_eq!(d.shapes().len(), shapes, "no new shape");
        let body =
            d.shapes().iter().find(|s| matches!(s.ph_type(), Some(PhType::Body | PhType::Obj))).and_then(|s| s.text.as_ref()).map(TextBody::text);
        assert_eq!(body.as_deref(), Some("One\nTwo"));
    }

    /// A content placeholder that holds a picture or a table (as a PowerPoint file brings one: `obj`, no text body) is no
    /// place for text: Nora's text goes to the empty body placeholder after them.
    #[test]
    fn typing_skips_content_placeholders_that_hold_a_picture_or_a_table() {
        let mut a = deck();
        a.run("slide.new", json!({"title": "Results"})).unwrap();
        a.session
            .edit(|doc, sel| {
                let placeholder = |idx| Some(deckcraft_model::Placeholder { kind: PhType::Obj, idx, ..Default::default() });
                let picture = Shape {
                    id: ShapeId(doc.alloc_id().expect("an id is free")),
                    ph: placeholder(10),
                    kind: ShapeKind::Picture { fill: Default::default() },
                    xfrm: Some(deckcraft_model::Xfrm::new(60.0, 120.0, 200.0, 120.0)),
                    ..Default::default()
                };
                let table = Shape {
                    id: ShapeId(doc.alloc_id().expect("an id is free")),
                    ph: placeholder(11),
                    kind: ShapeKind::Table(deckcraft_model::Table::new(2, 2, 300.0, 30.0)),
                    xfrm: Some(deckcraft_model::Xfrm::new(300.0, 120.0, 300.0, 60.0)),
                    ..Default::default()
                };
                let shapes = deckcraft_engine::shapes_mut(doc, sel).expect("the slide");
                // After the title, before the slide's empty body placeholder.
                shapes.insert(1, table);
                shapes.insert(1, picture);
                Ok(())
            })
            .unwrap();
        deselect(&mut a);
        let typed = run(&mut a, type_text("Hello", false)).unwrap();
        assert_eq!(typed["newTextBox"], false);
        let d = a.session.active().unwrap();
        let body = d.shapes().iter().find(|s| matches!(s.kind, ShapeKind::Shape) && matches!(s.ph_type(), Some(PhType::Body | PhType::Obj)));
        assert_eq!(body.and_then(|s| s.text.as_ref()).map(TextBody::text).as_deref(), Some("Hello"), "typed into the empty body");
    }

    /// The graphics a slide's layout draws on it (a stripe, a band, a logo) count as covered, as the renderer draws them.
    #[test]
    fn a_new_text_box_avoids_what_the_layout_draws_on_the_slide() {
        let mut a = deck();
        a.run("slide.new", json!({"layout": "titleOnly", "title": "Stripe"})).unwrap();
        let (layout, width) = {
            let d = a.session.active().unwrap();
            let id = d.current_slide().unwrap().layout;
            let master = d.doc.masters.first().unwrap();
            (master.layouts.iter().position(|l| l.id == id).unwrap(), d.doc.slide_size.width)
        };
        // A stripe across the middle of the Title Only layout, drawn on this slide.
        a.run("view.slideMaster", json!({"master": 0, "layout": layout})).unwrap();
        a.run("shape.insert", json!({"preset": "rect", "rect": [0.0, 200.0, width, 60.0]})).unwrap();
        a.run("view.closeMaster", json!({})).unwrap();
        deselect(&mut a);
        deselect(&mut a);
        assert_eq!(a.session.active().unwrap().current_slide().unwrap().title(), "Stripe", "back on the slide");
        run(&mut a, type_text("A word from Nora", false)).unwrap();
        let d = a.session.active().unwrap();
        let x = d.shapes().last().map(|s| deckcraft_engine::cmd::xfrm_of(&d.doc, &d.selection, s)).unwrap();
        assert!(x.y >= 260.0, "below the layout's stripe, in the tallest free strip: {x:?}");
    }

    /// An action that changed nothing says so (`"changed": false`) and leaves nothing to undo, as the conversation then
    /// keeps nothing to undo for it (craft-nora from 9d28340); one that changed the presentation doesn't.
    #[test]
    fn an_action_that_changed_nothing_says_so() {
        let mut a = deck();
        let read = run(&mut a, execute("slide.inspect", json!({"index": 0}))).unwrap();
        assert_eq!(read["changed"], false, "{read}");
        assert!(read["shapes"].is_array(), "the command's own answer is kept");
        let went = run(&mut a, execute("slide.go", json!({"index": 1}))).unwrap();
        assert_eq!((went["index"].clone(), went["changed"].clone()), (json!(1), json!(false)), "the current slide is not the presentation");
        let deselected = run(&mut a, execute("edit.deselect", json!({}))).unwrap();
        assert_eq!(deselected, json!({"changed": false}), "a command that answers nothing answers the mark");
        assert!(a.nora.changes.is_empty(), "nothing to undo");
        let made = run(&mut a, execute("slide.new", json!({"title": "New"}))).unwrap();
        assert!(made.get("changed").is_none(), "{made}");
        let typed = run(&mut a, type_text("Hi", false)).unwrap();
        assert!(typed.get("changed").is_none(), "{typed}");
        assert_eq!(a.nora.changes.len(), 2, "the two changes are Nora's to undo");
        // A batch answers its list of results as before.
        let batch = run(&mut a, DocActionKind::Batch { commands: vec![call("slide.inspect", json!({}))] }).unwrap();
        assert!(batch.is_array(), "{batch}");
    }

    /// A slide with no free strip keeps the place the new box had: across the middle.
    #[test]
    fn with_no_room_the_new_box_goes_across_the_middle() {
        let mut a = deck();
        let size = a.session.active().unwrap().doc.slide_size;
        a.run("shape.insert", json!({"preset": "rect", "rect": [0.0, 0.0, size.width, size.height]})).unwrap();
        deselect(&mut a);
        deselect(&mut a);
        run(&mut a, type_text("Hi", false)).unwrap();
        let d = a.session.active().unwrap();
        let x = d.shapes().last().map(|s| deckcraft_engine::cmd::xfrm_of(&d.doc, &d.selection, s)).unwrap();
        assert!((x.y - (size.height - 36.0) / 2.0).abs() < 0.5, "{x:?}");
    }

    #[test]
    fn a_slide_can_be_rendered_for_nora_to_look_at() {
        let mut a = deck();
        let seen = run(&mut a, DocActionKind::RenderPage { page: 2, scale: 0.5 }).unwrap();
        assert!(seen["png"].as_str().is_some_and(|png| png.starts_with("iVBOR")), "a base64 PNG");
        assert!(seen["width"].as_u64().is_some_and(|width| width > 0));
        assert_eq!((seen["slide"].clone(), seen["slides"].clone()), (json!(2), json!(2)));
        let error = run(&mut a, DocActionKind::RenderPage { page: 3, scale: 1.0 }).unwrap_err();
        assert!(error.contains("no slide 3"), "{error}");
    }

    #[test]
    fn save_is_the_app_s_and_says_whether_the_presentation_was_saved() {
        // A hosting page that takes the presentation.
        let services = Services {
            host_save: Some(Box::new(|d: &mut DocState| {
                d.saved_doc = Arc::clone(&d.doc);
                Ok(())
            })),
            ..Services::default()
        };
        let mut a = app(services);
        a.run("slide.new", json!({})).unwrap();
        assert!(a.session.active().unwrap().is_dirty());
        assert_eq!(run(&mut a, DocActionKind::Save).unwrap(), json!({"saved": true, "name": "Presentation1"}));
        assert!(!a.session.active().unwrap().is_dirty());
        // A page that refuses: Nora hears why.
        let services = Services { host_save: Some(Box::new(|_: &mut DocState| Err("the Drive is full".into()))), ..Services::default() };
        let mut a = app(services);
        a.run("slide.new", json!({})).unwrap();
        let error = run(&mut a, DocActionKind::Save).unwrap_err();
        assert!(error.contains("was not saved") && error.contains("the Drive is full"), "{error}");
        // Nowhere to save it (no file, no page, no dialog): not saved either.
        let mut a = app(Services::default());
        a.run("slide.new", json!({})).unwrap();
        assert!(run(&mut a, DocActionKind::Save).unwrap_err().contains("was not saved"));
    }

    #[test]
    fn nora_waits_while_the_pointer_changes_the_presentation() {
        let mut a = deck();
        let before = Arc::clone(&a.session.active().unwrap().doc);
        let drag = deckcraft_engine::Interaction { label: "Move".into(), doc: Arc::clone(&before), selection: Default::default() };
        a.session.active_mut().unwrap().interaction = Some(drag);
        let ctx = egui::Context::default();
        let action = DocumentCall { id: 1, kind: execute("slide.new", json!({})) };
        perform(&mut a, vec![Output::Document(action)], &ctx);
        assert!(Arc::ptr_eq(&a.session.active().unwrap().doc, &before), "nothing changes under the person's pointer");
        assert!(released(&mut a).is_empty(), "still dragging");
        a.session.active_mut().unwrap().interaction = None;
        let outputs = released(&mut a);
        assert_eq!(outputs.len(), 1);
        perform(&mut a, outputs, &ctx);
        assert_eq!(slides(&a), 3, "played once the pointer let go");
        assert!(a.nora.waiting.is_empty());
    }

    /// The sentences of the pane are words of the person's language, not the core's English.
    #[test]
    fn actions_are_described_in_the_pane_s_words() {
        assert_eq!(describe(Lang::En, &DocActionKind::Save), "Save the presentation");
        assert_eq!(describe(Lang::Fr, &DocActionKind::Save), "Enregistrer la présentation");
        assert_eq!(describe(Lang::De, &execute("format.bold", json!({}))), "format.bold ausführen");
        let one = vec![call("slide.new", json!({}))];
        assert_eq!(describe(Lang::En, &DocActionKind::Batch { commands: one.clone() }), "Run slide.new", "a batch of one names its command");
        assert_eq!(describe(Lang::Fr, &DocActionKind::Batch { commands: [one.clone(), one].concat() }), "Exécuter 2 commandes");
        assert_eq!(describe(Lang::It, &DocActionKind::RenderPage { page: 3, scale: 1.0 }), "Consultare la diapositiva 3");
        let typed = describe(Lang::En, &type_text(&"é".repeat(60), false));
        assert!(typed.starts_with("Type \"") && typed.ends_with("…\""), "{typed}");
        assert_eq!((labels(Lang::Fr).describe)(&DocActionKind::Observe), "Lire la présentation", "the panel's own function says the same");
    }

    /// While Nora works, the pane's Send button is a Stop button: a click ends her turn there and then, the pane says
    /// so, and the next message can go.
    #[test]
    fn stop_ends_nora_s_turn() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"on": true})).unwrap();
        let mut h = harness(a);
        start(&mut h, &outbox, &inbox, "Add a slide");
        assert!(h.state().nora.session.is_busy(), "Nora is at work");
        h.get_by_label("Stop").click();
        h.run_steps(4);
        assert!(!h.state().nora.session.is_busy(), "her turn ended");
        assert!(h.query_by_label("Stopped.").is_some(), "the pane says Nora was stopped");
        assert!(h.query_by_label("Send").is_some(), "the next message can go");
    }

    /// A call that got no answer at all is said in the pane's language, not with the browser's English reason.
    #[test]
    fn no_answer_from_nora_is_said_in_the_pane_s_language() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.nora.lang = Lang::Fr;
        a.run("ui.nora", json!({"on": true})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Bonjour");
        h.run_steps(2);
        h.get_by_label("Envoyer").click();
        h.run_steps(4);
        let sent = take(&outbox);
        let body = json!({"exception": "No answer from Nora: TypeError: Failed to fetch"}).to_string();
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push(HttpResponse { id: sent[0].id, status: 0, body });
        h.run_steps(4);
        assert!(h.query_by_label("Nora n'a pas pu être jointe. Vérifiez la connexion et réessayez.").is_some());
        assert!(h.query_by_label("No answer from Nora: TypeError: Failed to fetch").is_none());
        assert_eq!(error_worder(Lang::De)(&NoraError::Http(403)), "Ihre Sitzung ist abgelaufen. Bitte melden Sie sich erneut an.");
        assert_eq!(error_worder(Lang::It)(&NoraError::Http(502)), "Il server non ha potuto rispondere (errore 502). Riprovi tra poco.");
    }
}
