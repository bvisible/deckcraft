// //// Neoffice — added file (no upstream equivalent)
//! The product name the interface shows.
//!
//! Neoffice ships this editor under its own name. Our browser builds set `NEOFFICE_PRODUCT_NAME`
//! at compile time (the build step of `.github/workflows/neoffice-web.yml`); without it, as in
//! upstream's builds and every test run, [`name`] is upstream's name and nothing changes.
//!
//! DeckCraft has no string catalog, so the few places that show the app's name call [`name`],
//! [`about`] or [`label`]. Names that are stored or compared keep upstream's: the `DeckCraft`
//! theme, storage folders, the `deckcraft:*` messages of a hosting page, the file format's
//! generator and the licence line.

use std::sync::OnceLock;

/// Upstream's name for this app.
pub const UPSTREAM: &str = "DeckCraft";

/// The name the interface shows: `NEOFFICE_PRODUCT_NAME` when the build sets it (not blank),
/// upstream's otherwise.
pub fn name() -> &'static str {
    match option_env!("NEOFFICE_PRODUCT_NAME").map(str::trim) {
        Some(ours) if !ours.is_empty() => ours,
        _ => UPSTREAM,
    }
}

/// `About <name>`: the Help menu's item, the command palette's and the dialog's title.
pub fn about() -> &'static str {
    static ABOUT: OnceLock<String> = OnceLock::new();
    ABOUT.get_or_init(|| format!("About {}", name()))
}

/// One of upstream's fixed labels as the interface shows it: the app's name and `About DeckCraft`
/// become ours, any other label stays as it is.
pub fn label(upstream: &'static str) -> &'static str {
    match upstream {
        UPSTREAM => name(),
        "About DeckCraft" => about(),
        _ => upstream,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_the_variable_nothing_changes() {
        if option_env!("NEOFFICE_PRODUCT_NAME").is_none_or(|n| n.trim().is_empty()) {
            assert_eq!(name(), UPSTREAM);
            assert_eq!(about(), "About DeckCraft");
            assert_eq!(label("About DeckCraft"), "About DeckCraft");
        }
        assert_eq!(label("Command Palette"), "Command Palette");
        assert_eq!(label(UPSTREAM), name());
        assert_eq!(label("About DeckCraft"), about());
    }
}
