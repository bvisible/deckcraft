use super::{Icon, ShapeKind, SlideApp, Ui, big, big_button, group, json, rows, run};
use crate::neoffice_i18n::l10n; // //// Neoffice — the interface in the person's language (l10n), see `neoffice_i18n`.

pub(super) fn playback(app: &mut SlideApp, ui: &mut Ui) {
    let m = app
        .session
        .active()
        .and_then(|d| d.selected_shapes().into_iter().find_map(|s| if let ShapeKind::Media(m) = &s.kind { Some(m.clone()) } else { None }));
    group(ui, |ui| {
        let playing = app.session.active().and_then(|d| d.selection.shapes.first().copied()).is_some_and(|id| app.media.is_playing(id));
        if big_button(ui, if playing { Icon::Pause } else { Icon::Play }, if playing { "Pause" } else { "Play" }, m.is_some()).clicked() {
            run(app, "media.toggle", json!({}));
        }
        big(app, ui, Icon::TrimMedia, "Trim", "app.dialog", json!({"id": "trim"}));
    });
    group(ui, |ui| {
        rows(ui, |ui| {
            let mut auto = m.as_ref().is_some_and(|x| x.autoplay);
            if ui.checkbox(&mut auto, l10n("Start Automatically")).changed() {
                // //// Neoffice — in the person's language (l10n)
                run(app, "media.options", json!({"autoplay": auto}));
            }
            let mut across = m.as_ref().is_some_and(|x| x.play_across_slides);
            if ui.checkbox(&mut across, l10n("Play Across Slides")).changed() {
                // //// Neoffice — in the person's language (l10n)
                run(app, "media.options", json!({"acrossSlides": across}));
            }
            let mut lp = m.as_ref().is_some_and(|x| x.loop_play);
            if ui.checkbox(&mut lp, l10n("Loop until Stopped")).changed() {
                // //// Neoffice — in the person's language (l10n)
                run(app, "media.options", json!({"loop": lp}));
            }
        });
        rows(ui, |ui| {
            let mut hide = m.as_ref().is_some_and(|x| x.hide_while_not_playing);
            if ui.checkbox(&mut hide, l10n("Hide During Show")).changed() {
                // //// Neoffice — in the person's language (l10n)
                run(app, "media.options", json!({"hide": hide}));
            }
            let mut rw = m.as_ref().is_some_and(|x| x.rewind);
            if ui.checkbox(&mut rw, l10n("Rewind after Playing")).changed() {
                // //// Neoffice — in the person's language (l10n)
                run(app, "media.options", json!({"rewind": rw}));
            }
            let mut fs = m.as_ref().is_some_and(|x| x.full_screen);
            if ui.checkbox(&mut fs, l10n("Play Full Screen")).changed() {
                // //// Neoffice — in the person's language (l10n)
                run(app, "media.options", json!({"fullScreen": fs}));
            }
        });
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Volume, "Volume", m.is_some());
        egui::Popup::menu(&r).show(|ui| {
            for (l, v) in [("Low", 0.25), ("Medium", 0.5), ("High", 1.0), ("Mute", 0.0)] {
                if ui.button(l10n(l)).clicked() {
                    // //// Neoffice — in the person's language (l10n)
                    run(app, "media.options", json!({"volume": v}));
                }
            }
        });
    });
}
