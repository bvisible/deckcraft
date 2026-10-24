//! Quick Access Toolbar: the command buttons at the left of the title bar, which the user can
//! add to (right-click a ribbon button, or the toolbar's Customize menu), remove from and reset.

use egui::Ui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::SlideApp;
use crate::icons::Icon;
// //// Neoffice — the interface in the person's language: DeckCraft has no catalog, see `neoffice_i18n`.
use crate::neoffice_i18n::l10n;

/// One toolbar button: a command, its parameters, and how it looks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QatItem {
    pub id: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub params: Value,
    pub label: String,
    /// An [`Icon::name`].
    pub icon: String,
}

impl Default for QatItem {
    fn default() -> Self {
        QatItem { id: String::new(), params: Value::Null, label: String::new(), icon: Icon::More.name().into() }
    }
}

impl QatItem {
    pub fn new(id: &str, label: &str, icon: Icon) -> Self {
        QatItem { id: id.into(), params: Value::Null, label: label.into(), icon: icon.name().into() }
    }
    fn icon(&self) -> Icon {
        Icon::from_name(&self.icon).unwrap_or(Icon::More)
    }
    fn same(&self, id: &str, params: &Value) -> bool {
        self.id == id && (self.params == *params || (self.params.is_null() && params.as_object().is_some_and(|o| o.is_empty())))
    }
}

/// The commands the Customize menu offers as checkboxes, in PowerPoint's order.
pub fn common() -> Vec<QatItem> {
    vec![
        QatItem::new("app.home", "Home", Icon::Home),
        QatItem::new("file.new", "New", Icon::File),
        QatItem::new("app.openDialog", "Open", Icon::Folder),
        QatItem::new("file.save", "Save", Icon::Save),
        QatItem::new("app.print", "Print", Icon::Print),
        QatItem::new("edit.undo", "Undo", Icon::Undo),
        QatItem::new("edit.redo", "Redo", Icon::Redo),
        QatItem::new("show.fromStart", "Start from Beginning", Icon::PlayFromStart),
        QatItem::new("app.find", "Find", Icon::Search),
        QatItem::new("review.spelling", "Spelling", Icon::Spelling),
        QatItem::new("app.exportDialog", "Export", Icon::Export),
    ]
}

/// The toolbar a new install starts with.
pub fn defaults() -> Vec<QatItem> {
    // Home, Save, Export, Undo, Redo, Start from Beginning: the title bar's buttons before it was customizable.
    let all = common();
    ["app.home", "file.save", "app.exportDialog", "edit.undo", "edit.redo", "show.fromStart"]
        .iter()
        .filter_map(|id| all.iter().find(|q| q.id == *id).cloned())
        .collect()
}

/// `app.qat`: `{add: id, params?, label?, icon?} | {remove: id, params?} | {reset: true}` → the toolbar.
pub fn command(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let params = p.get("params").cloned().unwrap_or(Value::Null);
    if p.get("reset").and_then(Value::as_bool) == Some(true) {
        app.ui.qat = defaults();
    }
    if let Some(id) = p.get("add").and_then(Value::as_str) {
        if !crate::ribbon::known(id) {
            return Err(format!("unknown command `{id}`"));
        }
        if !app.ui.qat.iter().any(|q| q.same(id, &params)) {
            let base = common().into_iter().find(|q| q.id == id);
            let label = p.get("label").and_then(Value::as_str).map(String::from).or_else(|| base.as_ref().map(|b| b.label.clone()));
            let label = label.or_else(|| deckcraft_engine::find_command(id).map(|c| c.label.to_string())).unwrap_or_else(|| id.to_string());
            let icon = p.get("icon").and_then(Value::as_str).and_then(Icon::from_name).or_else(|| base.map(|b| b.icon())).unwrap_or(Icon::More);
            app.ui.qat.push(QatItem { id: id.into(), params: params.clone(), label, icon: icon.name().into() });
        }
    }
    if let Some(id) = p.get("remove").and_then(Value::as_str) {
        app.ui.qat.retain(|q| !q.same(id, &params));
    }
    serde_json::to_value(&app.ui.qat).map_err(|e| e.to_string())
}

fn run(app: &mut SlideApp, q: &QatItem) {
    match q.id.as_str() {
        "file.save" => app.save(),
        "show.fromStart" => app.start_show(app.show_start_index(), false),
        "review.spelling" => app.dialog = Some(crate::dialogs::Dialog::new("spelling")),
        id => {
            let p = if q.params.is_null() { json!({}) } else { q.params.clone() };
            let _ = app.run(id, p);
        }
    }
}

fn tip(q: &QatItem) -> String {
    let sc =
        deckcraft_engine::find_command(&q.id).and_then(|c| c.shortcut).or_else(|| crate::UI_COMMANDS.iter().find(|c| c.0 == q.id).and_then(|c| c.2));
    match sc {
        Some(s) => format!("{} ({})", l10n(&q.label), crate::ribbon::pretty_shortcut(s)), // //// Neoffice — in the person's language (l10n)
        None => l10n(&q.label).to_string(),                                               // //// Neoffice — in the person's language (l10n)
    }
}

/// Draw the toolbar and its Customize menu into a left-to-right row of the title bar.
pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    // //// Neoffice — a hosting page opens the presentations (`Services::host_owns_documents`): the buttons that would
    // bring another one in or show the start screen (Home, New, Open: `hidden_from_a_host`) are left out, here and in
    // the Customize menu. The person's own toolbar is not changed: the same buttons come back in a build without a host.
    let hosted = app.services.host_owns_documents;
    let offered = |id: &str| !(hosted && crate::hidden_from_a_host(id));
    let items: Vec<QatItem> = app.ui.qat.iter().filter(|q| offered(&q.id)).cloned().collect();
    for q in &items {
        let on = q.id == "app.home" && app.backstage_open();
        let en = match q.id.as_str() {
            "app.home" => true,
            "file.save" | "show.fromStart" | "review.spelling" | "app.find" => app.session.active().is_some(),
            id => crate::ribbon::enabled(app, id),
        };
        let r = crate::widgets::icon_toggle(ui, q.icon(), &tip(q), en, on);
        if r.clicked() {
            run(app, q);
        }
        r.context_menu(|ui| {
            if ui.button(l10n("Remove from Quick Access Toolbar")).clicked() {
                // //// Neoffice — in the person's language (l10n)
                let _ = command(app, &json!({"remove": q.id, "params": q.params}));
                ui.close();
            }
        });
    }
    let r = crate::widgets::icon_toggle(ui, Icon::ChevronDown, "Customize Quick Access Toolbar", true, false);
    egui::Popup::menu(&r).show(|ui| {
        ui.label(egui::RichText::new(l10n("Customize Quick Access Toolbar")).strong()); // //// Neoffice — in the person's language (l10n)
        // //// Neoffice — see `offered` above.
        for c in common().into_iter().filter(|c| offered(&c.id)) {
            let mut on = app.ui.qat.iter().any(|q| q.same(&c.id, &Value::Null));
            if ui.checkbox(&mut on, l10n(&c.label)).changed() {
                // //// Neoffice — in the person's language (l10n)
                let key = if on { "add" } else { "remove" };
                let _ = command(app, &json!({key: c.id}));
            }
        }
        // //// Neoffice — see `offered` above.
        let extra: Vec<QatItem> =
            app.ui.qat.iter().filter(|q| offered(&q.id) && !common().iter().any(|c| q.same(&c.id, &Value::Null))).cloned().collect();
        if !extra.is_empty() {
            ui.separator();
            for q in extra {
                let mut on = true;
                if ui.checkbox(&mut on, l10n(&q.label)).changed() {
                    // //// Neoffice — in the person's language (l10n)
                    let _ = command(app, &json!({"remove": q.id, "params": q.params}));
                }
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(l10n("Right-click any ribbon button to add it.")).weak()); // //// Neoffice — in the person's language (l10n)
        if ui.button(l10n("Reset to Defaults")).clicked() {
            // //// Neoffice — in the person's language (l10n)
            let _ = command(app, &json!({"reset": true}));
            ui.close();
        }
    });
}

/// The ribbon buttons' right-click menu: add the button to the toolbar.
pub fn ribbon_menu(app: &mut SlideApp, r: &egui::Response, icon: Icon, label: &str, id: &str, p: &Value) {
    r.context_menu(|ui| {
        let has = app.ui.qat.iter().any(|q| q.same(id, p));
        let text = if has { "Remove from Quick Access Toolbar" } else { "Add to Quick Access Toolbar" };
        if ui.button(l10n(text)).clicked() {
            // //// Neoffice — in the person's language (l10n)
            let label = label.replace('\n', " ");
            let key = if has { "remove" } else { "add" };
            let _ = command(app, &json!({key: id, "params": p, "label": label, "icon": icon.name()}));
            ui.close();
        }
    });
}
