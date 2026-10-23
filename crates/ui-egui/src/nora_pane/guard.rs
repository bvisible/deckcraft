// //// Neoffice — added file (no upstream equivalent): what a command of Nora's really wrote, read before it stays.

//! The links a change of Nora's left in the presentation, read once the change is made.
//!
//! The Neoffice server refuses a link to a file in what Nora asks for, but it reads only the parameters
//! of a command: a command can leave a link it was not given in so many words (a slide reused with its
//! links, a link copied with its text, a format painted). DeckCraft writes a link's address as it is, and
//! PowerPoint opens a link to a file or starts a program from it. So after every change of Nora's the
//! pane reads the links of what the change touched (the slides and the masters, their shapes, text, notes
//! and tables): an address that is no web page or e-mail address ([`craft_nora::links::web_or_mail`]), or
//! a program to start, takes the change back when the presentation holds more of it than before. A link
//! to a slide, to the next or the last one, to the end of the show, stays.
//!
//! What was already there (the person's own links, moved or not) is counted, not refused: only a change
//! that adds to it is.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use craft_nora::links;
use deckcraft_model::Presentation;
use serde_json::{Map, Value};

/// The longest address quoted back to Nora.
const MAX_QUOTED_CHARS: usize = 200;

/// The most of them listed in one refusal.
const MAX_LISTED: usize = 5;

/// How deep a part is read: a presentation's own groups stop far above.
const MAX_DEPTH: usize = 256;

/// One link the presentation holds that leads outside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reach {
    /// What it is, to count it wherever it is: its address or its program.
    key: String,
    /// What Nora is told it is.
    pub(super) what: String,
    /// Where it is: a slide, a master.
    pub(super) place: String,
}

/// What `after` holds that leads outside the presentation more often than `before` held it: what a
/// change of Nora's added. Only the slides and masters the change touched are read first (one it left
/// alone is the very same); the whole presentation is counted only when one of them holds something.
pub(super) fn added(before: &Presentation, after: &Presentation) -> Vec<Reach> {
    let mut touched = Vec::new();
    for (at, slide) in after.slides.iter().enumerate() {
        if !before.slides.get(at).is_some_and(|old| Arc::ptr_eq(old, slide)) {
            read(slide.as_ref(), &format!("slide {}", at + 1), &mut touched);
        }
    }
    for (at, master) in after.masters.iter().enumerate() {
        if !before.masters.get(at).is_some_and(|old| Arc::ptr_eq(old, master)) {
            read(master.as_ref(), &format!("slide master {}", at + 1), &mut touched);
        }
    }
    for (old, new, place) in
        [(&before.notes_master, &after.notes_master, "the notes master"), (&before.handout_master, &after.handout_master, "the handout master")]
    {
        let same = match (old, new) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if !same && let Some(new) = new {
            read(new.as_ref(), place, &mut touched);
        }
    }
    if touched.is_empty() {
        return Vec::new();
    }
    let mut held: BTreeMap<String, usize> = BTreeMap::new();
    for reach in all(before) {
        *held.entry(reach.key).or_default() += 1;
    }
    let mut new = Vec::new();
    for reach in all(after) {
        match held.get_mut(&reach.key) {
            Some(count) if *count > 0 => *count -= 1,
            _ => new.push(reach),
        }
    }
    new
}

/// What Nora is told when her change is taken back: what `what` wrote, and what she may write instead.
pub(super) fn refusal(what: &str, found: &[Reach]) -> String {
    let mut listed: Vec<String> = found.iter().take(MAX_LISTED).map(|reach| format!("{} ({})", reach.what, reach.place)).collect();
    if found.len() > MAX_LISTED {
        listed.push(format!("and {} more", found.len() - MAX_LISTED));
    }
    format!(
        "{what} was undone, nothing of it is left: it wrote {}. A link of yours goes to a web page, an e-mail address or a slide of this \
presentation (`slide`), never to a file or a program. Tell the person; do not look for another way to write it.",
        listed.join("; ")
    )
}

/// Every link `deck` holds that leads outside it.
fn all(deck: &Presentation) -> Vec<Reach> {
    let mut found = Vec::new();
    for (at, slide) in deck.slides.iter().enumerate() {
        read(slide.as_ref(), &format!("slide {}", at + 1), &mut found);
    }
    for (at, master) in deck.masters.iter().enumerate() {
        read(master.as_ref(), &format!("slide master {}", at + 1), &mut found);
    }
    for (master, place) in [(&deck.notes_master, "the notes master"), (&deck.handout_master, "the handout master")] {
        if let Some(master) = master {
            read(master.as_ref(), place, &mut found);
        }
    }
    found
}

/// The links of one part, read in the form the presentation is saved in (`.deckcraft`): every link of
/// a shape, of a run of text, of a table's cell is a `Hyperlink`, `{action: {kind, …}, tooltip, …}`.
/// A part that cannot be read so is counted as one reach of its own kind, the same before and after.
/// Each address counts once per part: a link is on every run of its text, and a run split in two (a word
/// of the person's linked text made bold) is still the one link.
fn read<T: serde::Serialize>(part: &T, place: &str, found: &mut Vec<Reach>) {
    let mut here = Vec::new();
    match serde_json::to_value(part) {
        Ok(value) => walk(&value, place, &mut here, 0),
        Err(_) => here.push(Reach { key: "unreadable".to_string(), what: "a part that could not be read".to_string(), place: place.to_string() }),
    }
    let mut seen = HashSet::new();
    found.extend(here.into_iter().filter(|reach| seen.insert(reach.key.clone())));
}

fn walk(value: &Value, place: &str, found: &mut Vec<Reach>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(action)) = map.get("action") {
                check(action, place, found);
            }
            for item in map.values() {
                walk(item, place, found, depth + 1);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, place, found, depth + 1);
            }
        }
        _ => {}
    }
}

/// A link's action (`deckcraft_model::text::Action`): an address must be a web page or an e-mail
/// address, and a program is never started from a link of Nora's.
fn check(action: &Map<String, Value>, place: &str, found: &mut Vec<Reach>) {
    let text = |key: &str| action.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string();
    match action.get("kind").and_then(Value::as_str) {
        Some("url") => {
            let url = text("url");
            if !url.is_empty() && !links::web_or_mail(&url) {
                found.push(Reach { what: format!("a link to {}", quoted(&url)), key: format!("url:{url}"), place: place.to_string() });
            }
        }
        Some("program") => {
            let path = text("path");
            found.push(Reach {
                what: format!("a link that starts the program {}", quoted(&path)),
                key: format!("program:{path}"),
                place: place.to_string(),
            });
        }
        _ => {}
    }
}

/// `text` on one line, cut at [`MAX_QUOTED_CHARS`].
fn quoted(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX_QUOTED_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(MAX_QUOTED_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use deckcraft_engine::Session;
    use serde_json::json;

    use super::*;

    /// A presentation with a slide and its title selected, and that presentation as it is.
    fn session() -> (Session, Arc<Presentation>) {
        let mut s = Session::with_new();
        let title = s.doc().unwrap().doc.slides[0].shapes[0].id.0;
        s.execute("edit.select", &json!({"ids": [title]})).unwrap();
        let doc = Arc::clone(&s.doc().unwrap().doc);
        (s, doc)
    }

    fn now(s: &Session) -> Arc<Presentation> {
        Arc::clone(&s.doc().unwrap().doc)
    }

    #[test]
    fn a_link_to_a_web_page_an_address_or_a_slide_is_no_reach() {
        let (mut s, before) = session();
        s.execute("insert.hyperlink", &json!({"url": "https://example.com/tarifs"})).unwrap();
        s.execute("shape.action", &json!({"action": "url", "url": "mailto:info@example.com", "on": "hover"})).unwrap();
        assert!(added(&before, &now(&s)).is_empty());
        s.execute("shape.action", &json!({"action": "next"})).unwrap();
        assert!(added(&before, &now(&s)).is_empty());
    }

    #[test]
    fn a_link_to_a_file_or_a_program_is_a_reach() {
        let (mut s, before) = session();
        s.execute("insert.hyperlink", &json!({"url": "C:\\Tarifs\\catalogue.pdf"})).unwrap();
        let found = added(&before, &now(&s));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].what, "a link to C:\\Tarifs\\catalogue.pdf");
        assert_eq!(found[0].place, "slide 1");
        let (mut s, before) = session();
        s.execute("shape.action", &json!({"action": "program", "path": "calc.exe"})).unwrap();
        let found = added(&before, &now(&s));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].what, "a link that starts the program calc.exe");
    }

    #[test]
    fn what_was_there_is_counted_not_refused() {
        let (mut s, _) = session();
        s.execute("insert.hyperlink", &json!({"url": "\\\\srv\\share\\x.pptx"})).unwrap();
        let before = now(&s);
        s.execute("slide.new", &json!({"title": "Two"})).unwrap();
        assert!(added(&before, &now(&s)).is_empty(), "another slide changed");
        s.execute("slide.duplicate", &json!({"index": 0})).unwrap();
        assert_eq!(added(&before, &now(&s)).len(), 1, "the link copied with its slide is one more");
    }

    #[test]
    fn a_link_split_over_two_runs_is_still_one_link() {
        let (mut s, _) = session();
        let title = s.doc().unwrap().doc.slides[0].shapes[0].id.0;
        s.execute("text.set", &json!({"id": title, "text": "le catalogue"})).unwrap();
        s.execute("edit.select", &json!({"ids": [title]})).unwrap();
        s.execute("insert.hyperlink", &json!({"url": "\\\\srv\\cat.pdf"})).unwrap();
        let before = now(&s);
        assert_eq!(all(&before).len(), 1, "one link on the slide");
        let mut split = (*before).clone();
        let mut slide = (*split.slides[0]).clone();
        let shape = slide.shapes.iter_mut().find(|shape| shape.click.is_some()).unwrap();
        let link = shape.click.clone();
        let mut copy = shape.clone();
        copy.click = link;
        slide.shapes.push(copy);
        split.slides[0] = Arc::new(slide);
        assert!(added(&before, &split).is_empty(), "the same address twice on one slide is still the one link");
    }

    #[test]
    fn the_refusal_says_what_and_where() {
        let (mut s, before) = session();
        s.execute("insert.hyperlink", &json!({"url": "file:///C:/x.pdf"})).unwrap();
        let text = refusal("`insert.hyperlink`", &added(&before, &now(&s)));
        assert!(text.starts_with("`insert.hyperlink` was undone"), "{text}");
        assert!(text.contains("a link to file:///C:/x.pdf (slide 1)"), "{text}");
    }
}
