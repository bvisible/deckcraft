// //// Neoffice — added file (no upstream equivalent): the account button of the title bar.

//! The account button at the right end of the title bar, the one WordCraft draws there: a round
//! badge in the accent colour with the initials of the person, and their name as the hover text.
//! The name is the one comments are signed with (`Prefs::author`), which a hosting page (NeoDesk,
//! the Drive of the Suite) sets from `?author=`. The button shows only when a name is known: not
//! while the name is blank or the engine's placeholder for a user nobody named.

use std::sync::LazyLock;

use egui::{Align2, Color32, Sense, Ui, vec2};

use crate::theme::{self, Tokens};

/// The title bar's control height (its Present button's), which the button is a square of.
pub const SIZE: f32 = 26.0;

/// The engine's name for a user nobody named ("Presenter").
static PLACEHOLDER: LazyLock<String> = LazyLock::new(|| deckcraft_engine::Prefs::default().author);

/// The person's name when one is known: not blank, and not the engine's placeholder.
pub fn known_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && name != PLACEHOLDER.as_str()).then_some(name)
}

/// Up to two initials: the first letters of the name's words, as WordCraft takes them.
pub fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect()
}

/// The colour of text on the accent (the initials here, the person's own messages in the Nora pane):
/// of white and the theme's dark, the one that reads better there. On the dark theme the dark is
/// the title bar's own colour (5:1, white 3:1); the light theme's bar is light, and none of its
/// greys reaches 4.5:1 on its orange (its text 4:1, white 3.9:1), so it is the theme's darkest ink,
/// the black of its caret (5.4:1).
pub(crate) fn ink(t: &Tokens) -> Color32 {
    if t.dark { t.chrome } else { Color32::BLACK }
}

/// Paints the button, with the name as its hover text. Nothing is drawn when no name is known. Like
/// WordCraft's, the button has no accessible label of its own.
pub fn button(ui: &mut Ui, name: &str) {
    let Some(name) = known_name(name) else { return };
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(SIZE, SIZE), Sense::click());
    ui.painter().circle_filled(r.center(), SIZE / 2.0 - 1.0, t.accent);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, initials(name), theme::bold(10.5), ink(&t));
    resp.on_hover_text(name);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use egui::epaint::CircleShape;
    use egui::{Rect, Shape, Vec2};
    use egui_kittest::kittest::Queryable as _;

    use super::*;
    use crate::{Services, SlideApp};
    use deckcraft_engine::Session;

    type Harness = egui_kittest::Harness<'static, SlideApp>;

    const WIDTH: f32 = 1280.0;
    /// `egui_kittest` lays the UI out inside an 8 point margin.
    const HARNESS_MARGIN: f32 = 8.0;

    #[test]
    fn a_name_is_known_unless_blank_or_the_placeholder() {
        assert_eq!(known_name("  Jane Doe "), Some("Jane Doe"));
        assert_eq!(known_name(""), None);
        assert_eq!(known_name("   "), None);
        assert_eq!(known_name(&deckcraft_engine::Prefs::default().author), None, "the engine's default names nobody");
    }

    #[test]
    fn initials_are_the_first_letters_of_the_first_two_words() {
        assert_eq!(initials("Jane Doe"), "JD");
        assert_eq!(initials("Jane Mary Doe"), "JM");
        assert_eq!(initials("Jane"), "J");
        assert_eq!(initials("Élodie Ünal"), "ÉÜ");
        assert_eq!(initials("Jean-Luc Picard"), "JP");
        assert_eq!(initials(""), "");
    }

    /// WCAG contrast ratio of two opaque colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        fn luminance(c: Color32) -> f32 {
            let lin = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
        }
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn the_initials_read_on_the_accent_in_both_themes() {
        for t in [Tokens::light(), Tokens::dark()] {
            let r = contrast(ink(&t), t.accent);
            assert!(r >= 4.5, "dark={}: initials on the accent at {r:.2}:1", t.dark);
            assert!(r >= contrast(Color32::WHITE, t.accent), "dark={}: white would read better than {r:.2}:1", t.dark);
        }
    }

    fn harness(name: Option<&str>, dark: bool) -> Harness {
        let mut app = SlideApp::new(Session::with_new(), Services::default());
        if let Some(name) = name {
            app.session.prefs.author = name.into();
        }
        app.ui.brightness = if dark { theme::Brightness::Dark } else { theme::Brightness::Light };
        let mut h = Harness::builder().with_size(egui::vec2(WIDTH, 800.0)).with_step_dt(1.0 / 60.0).build_ui_state(
            |ui, app: &mut SlideApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
        // The app keeps requesting repaints, so step frames instead of run().
        h.run_steps(4);
        h
    }

    /// The badge: a circle in the accent colour in the title bar.
    fn badge(h: &Harness, accent: Color32) -> Option<CircleShape> {
        h.output().shapes.iter().find_map(|s| match &s.shape {
            Shape::Circle(c) if c.fill == accent && c.center.y < HARNESS_MARGIN + crate::ribbon::TITLE_H => Some(*c),
            _ => None,
        })
    }

    /// The square the badge fills (its circle keeps a point of margin).
    fn square(badge: &CircleShape) -> Rect {
        Rect::from_center_size(badge.center, Vec2::splat(2.0 * (badge.radius + 1.0)))
    }

    /// The painted text `text`: its rectangle, font size and colour.
    fn text(h: &Harness, text: &str) -> Option<(Rect, f32, Color32)> {
        h.output().shapes.iter().find_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.job.text == text => {
                let format = &t.galley.job.sections.first()?.format;
                Some((t.galley.rect.translate(t.pos.to_vec2()), format.font_id.size, format.color))
            }
            _ => None,
        })
    }

    #[test]
    fn the_title_bar_ends_with_the_initials_of_the_person_named() {
        for dark in [false, true] {
            let tokens = if dark { Tokens::dark() } else { Tokens::light() };
            let h = harness(Some("Jane Doe"), dark);
            // The badge, in the accent colour, fills a square of the bar's control height.
            let circle = badge(&h, tokens.accent).expect("the badge is painted");
            assert_eq!(circle.radius, SIZE / 2.0 - 1.0, "dark={dark}");
            let button = square(&circle);
            // At the right end of the title bar (its 10 point margin), centred in it.
            assert_eq!(WIDTH - HARNESS_MARGIN - button.right(), 10.0, "dark={dark}: the last control of the bar");
            let bar_middle = HARNESS_MARGIN + crate::ribbon::TITLE_H / 2.0;
            assert!((button.center().y - bar_middle).abs() <= 1.0, "dark={dark}: centred in the bar: {button:?}");
            // The initials sit on it, small and legible.
            let (at, size, color) = text(&h, "JD").expect("the initials are painted");
            assert!(button.contains(at.center()), "dark={dark}: {at:?} in {button:?}");
            assert_eq!(size, 10.5, "dark={dark}");
            assert_eq!(color, ink(&tokens), "dark={dark}");
            let ratio = contrast(color, tokens.accent);
            assert!(ratio >= 4.5, "dark={dark}: the initials read at {ratio:.2}:1");
        }
    }

    #[test]
    fn hovering_the_button_shows_the_name_and_nothing_else() {
        let mut h = harness(Some("Jane Doe"), false);
        let circle = badge(&h, Tokens::light().accent).expect("the badge is painted");
        assert!(h.query_by_label_contains("Jane Doe").is_none(), "the name is not shown before the pointer comes");
        h.hover_at(circle.center);
        // Past egui's tooltip delay (half a second).
        h.run_steps(60);
        assert!(text(&h, "Jane Doe").is_some(), "the tooltip is the name itself");
        assert!(h.query_by_role_and_label(egui::accesskit::Role::Label, "Jane Doe").is_some(), "the tooltip is the name itself");
    }

    #[test]
    fn no_button_without_a_known_name() {
        let accent = Tokens::light().accent;
        // No name (the engine's placeholder, "Presenter"), an empty one, a blank one.
        for name in [None, Some(""), Some("   ")] {
            let h = harness(name, false);
            assert!(badge(&h, accent).is_none(), "{name:?}: no badge");
            assert!(text(&h, "P").is_none(), "{name:?}: no initials");
        }
        // The name is read each frame: once a name comes, the button does.
        let mut h = harness(None, false);
        h.state_mut().session.prefs.author = "Ada Lovelace".into();
        h.run_steps(2);
        assert!(badge(&h, accent).is_some());
        assert!(text(&h, "AL").is_some());
    }
}
