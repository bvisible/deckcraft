use super::arrange::arrange_group;
use super::galleries::smartart_gallery;
use super::{Icon, SlideApp, Ui, Value, big_button, group, json, rows, run, small_button, small_button_ex, widgets};
use crate::neoffice_i18n::l10n; // //// Neoffice — the interface in the person's language (l10n), see `neoffice_i18n`.

/// SmartArt Design: Create Graphic (Text Pane, Add Shape, Promote, Demote, Remove) and Layouts.
pub(super) fn smartart_design(app: &mut SlideApp, ui: &mut Ui) {
    let sel = app.session.active().and_then(deckcraft_engine::cmd::smartart::selected);
    let row = app.ui.smartart_row;
    let (gid, n, kind, style, colors) = match &sel {
        Some((id, d)) => (Some(id.0), d.nodes.len(), d.kind.clone(), d.style.clone(), d.colors.clone()),
        None => (None, 0, String::new(), String::new(), String::new()),
    };
    let at = row.min(n.saturating_sub(1));
    group(ui, |ui| {
        if big_button(ui, Icon::SmartArt, "Text\nPane", gid.is_some()).on_hover_text(l10n("Type and organise the diagram's text")).clicked() {
            // //// Neoffice — in the person's language (l10n)
            run(app, "view.pane", json!({"pane": "smartart", "toggle": true}));
        }
        rows(ui, |ui| {
            let (main, arrow) = small_button_ex(ui, Icon::Plus, "Add Shape", gid.is_some(), false, true);
            if let Some(arrow) = arrow {
                widgets::split_menu(&main, &arrow, |ui| {
                    for (label, pos) in [
                        ("Add Shape After", "after"),
                        ("Add Shape Before", "before"),
                        ("Add Shape Above", "above"),
                        ("Add Shape Below", "below"),
                        ("Add Assistant", "assistant"),
                    ] {
                        if ui.button(l10n(label)).clicked() {
                            // //// Neoffice — in the person's language (l10n)
                            let index = if n == 0 { Value::Null } else { json!(at) };
                            run(app, "smartArt.addNode", json!({"id": gid, "index": index, "position": pos}));
                        }
                    }
                });
            }
            if small_button(ui, Icon::DecreaseIndent, "Promote", n > 0, false).clicked() {
                run(app, "smartArt.promote", json!({"id": gid, "index": at}));
            }
            if small_button(ui, Icon::IncreaseIndent, "Demote", n > 0, false).clicked() {
                run(app, "smartArt.demote", json!({"id": gid, "index": at}));
            }
        });
        let pictures = kind.starts_with("picture");
        // //// Neoffice — `small_button_tip`: its own tooltip (widgets::set_folded)
        if widgets::small_button_tip(
            ui,
            Icon::Picture,
            "Insert Picture",
            pictures && n > 0,
            false,
            l10n("Put a picture in the selected node's picture placeholder"),
        ) // //// Neoffice — in the person's language (l10n)
        .clicked()
        {
            run(app, "app.smartArtPictureDialog", json!({"id": gid, "index": at}));
        }
    });
    group(ui, |ui| {
        let (main, arrow) = small_button_ex(ui, Icon::Layout, "Layouts", gid.is_some(), false, true);
        let outline = sel.as_ref().map(|(_, d)| d.nodes.clone());
        if let Some(arrow) = arrow {
            widgets::split_menu(&main, &arrow, |ui| {
                smartart_gallery(app, ui, "smartArt.layout", json!({"id": gid}), &kind, (&style, &colors), outline.as_deref());
            });
        }
        let (main, arrow) = small_button_ex(ui, Icon::Layout, "Convert", gid.is_some(), false, true);
        if let Some(arrow) = arrow {
            widgets::split_menu(&main, &arrow, |ui| {
                if ui.button(l10n("Convert to Text")).clicked() {
                    // //// Neoffice — in the person's language (l10n)
                    run(app, "smartArt.toText", json!({"id": gid}));
                }
                if ui.button(l10n("Convert to Shapes")).clicked() {
                    // //// Neoffice — in the person's language (l10n)
                    run(app, "smartArt.toShapes", json!({"id": gid}));
                }
            });
        }
    });
    group(ui, |ui| {
        let (main, arrow) = small_button_ex(ui, Icon::ChangeChartType, "Change Colors", gid.is_some(), false, true);
        if let Some(arrow) = arrow {
            widgets::split_menu(&main, &arrow, |ui| {
                for (k, label) in deckcraft_engine::cmd::smartart::COLORS {
                    if matches!(k, "range23" | "accent1" | "accent1Grad") {
                        ui.separator();
                    }
                    let on = colors == k || (colors.is_empty() && k == "colorful");
                    if ui.selectable_label(on, l10n(label)).clicked() {
                        // //// Neoffice — in the person's language (l10n)
                        run(app, "smartArt.colors", json!({"id": gid, "colors": k}));
                    }
                }
            });
        }
        let (main, arrow) = small_button_ex(ui, Icon::SmartArt, "SmartArt Styles", gid.is_some(), false, true);
        if let Some(arrow) = arrow {
            widgets::split_menu(&main, &arrow, |ui| {
                for (k, label) in deckcraft_engine::cmd::smartart::STYLES {
                    if k == "polished" {
                        ui.separator();
                        ui.label(l10n("3-D")); // //// Neoffice — in the person's language (l10n)
                    }
                    let on = style == k || (style.is_empty() && k == "flat");
                    if ui.selectable_label(on, l10n(label)).clicked() {
                        // //// Neoffice — in the person's language (l10n)
                        run(app, "smartArt.style", json!({"id": gid, "style": k}));
                    }
                }
            });
        }
    });
    arrange_group(app, ui);
}
