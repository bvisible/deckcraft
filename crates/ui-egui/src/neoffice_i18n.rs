// //// Neoffice — added file (no upstream equivalent): the words Neoffice adds to DeckCraft, in the person's language.

//! The language of what Neoffice adds to DeckCraft: the Nora pane and its switch in Preferences.
//!
//! DeckCraft's own interface is English only: it has no string catalog yet (see `docs/localization-parity.md`). The
//! words Neoffice adds speak the person's language anyway: French, German or Italian when the system prefers one of
//! them (in the browser build, the browser's languages), English otherwise. As in WordCraft's catalogs, the strings in
//! the code are English and are the keys; a test checks that every language has every one of them. When DeckCraft gets
//! a catalog of its own, these tables join it, as WordCraft's `i18n/neoffice.rs` sits beside upstream's.
//!
//! French is written for Neoffice's Swiss customers, with "vous", and reuses the wording of WordCraft's Nora pane;
//! German is Swiss Standard German ("ss", never "ß").

use std::sync::OnceLock;

/// A language the words Neoffice added are shown in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lang {
    /// The source language, and the one for every language without words here.
    #[default]
    En,
    Fr,
    De,
    It,
}

impl Lang {
    /// Every language, English first.
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Fr, Lang::De, Lang::It];

    /// The language's BCP 47 code.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::It => "it",
        }
    }

    /// The language of a locale tag such as `fr`, `fr-CH`, `de_CH.UTF-8` or `it-IT`; `None` for a language that has no
    /// words here. `C` and `POSIX` are English.
    pub fn from_tag(tag: &str) -> Option<Lang> {
        let primary = tag.trim().split(['-', '_', '.', '@']).next().unwrap_or_default().to_ascii_lowercase();
        match primary.as_str() {
            "en" | "c" | "posix" => Some(Lang::En),
            "fr" => Some(Lang::Fr),
            "de" => Some(Lang::De),
            "it" => Some(Lang::It),
            _ => None,
        }
    }

    /// The first of the person's preferred languages that has words here, in their order, else English. Those are
    /// `DECKCRAFT_LANGUAGE` when it is set, then the operating system's preferred languages, or the browser's in the
    /// web build. Looked up once. Tests get English whatever the machine speaks.
    pub fn system() -> Lang {
        if cfg!(test) {
            return Lang::En;
        }
        static SYSTEM: OnceLock<Lang> = OnceLock::new();
        *SYSTEM.get_or_init(|| {
            let wanted = std::env::var("DECKCRAFT_LANGUAGE").ok().and_then(|tag| Lang::from_tag(&tag));
            wanted.or_else(|| sys_locale::get_locales().take(16).find_map(|tag| Lang::from_tag(&tag))).unwrap_or_default()
        })
    }

    /// The words of the language, English first.
    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::Fr => FRENCH,
            Lang::De => GERMAN,
            Lang::It => ITALIAN,
        }
    }
}

/// `english` in `lang`; English itself when the language has no entry for it.
pub fn tr(lang: Lang, english: &'static str) -> &'static str {
    lang.table().iter().find(|(key, _)| *key == english).map_or(english, |(_, word)| *word)
}

/// Fills the `{name}` placeholders of a looked-up string; translations may put them in another order. A placeholder
/// without a value is left as it is.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, after_open)) = rest.split_once('{') {
        out.push_str(before);
        let filled = after_open.split_once('}').and_then(|(name, after)| args.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value, after)));
        match filled {
            Some((value, after)) => {
                out.push_str(value);
                rest = after;
            }
            None => {
                out.push('{');
                rest = after_open;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Every string Neoffice added, in English: the keys of each language's table, in this order.
pub const ENGLISH: &[&str] = &[
    "Nora isn't available in this version yet",
    "New conversation",
    "Undo",
    "Undo the last change Nora made to the presentation",
    "Ask Nora…",
    "Send",
    "Nora is working…",
    "Ask Nora to write, rewrite, translate or format your presentation.",
    "Nora wants to:",
    "Allow",
    "Refuse",
    "Read the presentation",
    "Run {command}",
    "Run {count} commands",
    "Select \"{text}\"",
    "Type \"{text}\"",
    "Look at slide {slide}",
    "Save the presentation",
    "Undo the last change",
    "Assistant",
    "Show the Nora assistant pane (experimental)",
    "Nora has nothing left to undo.",
    "You changed the presentation after Nora did: use the editor's Undo instead.",
];

/// French (Swiss customers, "vous"), WordCraft's wording where the pane says the same.
const FRENCH: &[(&str, &str)] = &[
    ("Nora isn't available in this version yet", "Nora n'est pas encore disponible dans cette version"),
    ("New conversation", "Nouvelle conversation"),
    ("Undo", "Annuler"),
    ("Undo the last change Nora made to the presentation", "Annuler la dernière modification faite par Nora dans la présentation"),
    ("Ask Nora…", "Demander à Nora…"),
    ("Send", "Envoyer"),
    ("Nora is working…", "Nora travaille…"),
    (
        "Ask Nora to write, rewrite, translate or format your presentation.",
        "Demandez à Nora d'écrire, de réécrire, de traduire ou de mettre en forme votre présentation.",
    ),
    ("Nora wants to:", "Nora souhaite :"),
    ("Allow", "Autoriser"),
    ("Refuse", "Refuser"),
    ("Read the presentation", "Lire la présentation"),
    ("Run {command}", "Exécuter {command}"),
    ("Run {count} commands", "Exécuter {count} commandes"),
    ("Select \"{text}\"", "Sélectionner \"{text}\""),
    ("Type \"{text}\"", "Saisir \"{text}\""),
    ("Look at slide {slide}", "Consulter la diapositive {slide}"),
    ("Save the presentation", "Enregistrer la présentation"),
    ("Undo the last change", "Annuler la dernière modification"),
    ("Assistant", "Assistant"),
    ("Show the Nora assistant pane (experimental)", "Afficher le volet de l'assistant Nora (expérimental)"),
    ("Nora has nothing left to undo.", "Nora n'a plus rien à annuler."),
    (
        "You changed the presentation after Nora did: use the editor's Undo instead.",
        "Vous avez modifié la présentation après Nora : utilisez plutôt la fonction Annuler de l'éditeur.",
    ),
];

/// German (Swiss Standard German, "Sie"); the Undo button says what WordCraft's catalog says.
const GERMAN: &[(&str, &str)] = &[
    ("Nora isn't available in this version yet", "Nora ist in dieser Version noch nicht verfügbar"),
    ("New conversation", "Neues Gespräch"),
    ("Undo", "Rückgängig"),
    ("Undo the last change Nora made to the presentation", "Letzte Änderung von Nora an der Präsentation rückgängig machen"),
    ("Ask Nora…", "Nora fragen…"),
    ("Send", "Senden"),
    ("Nora is working…", "Nora arbeitet…"),
    (
        "Ask Nora to write, rewrite, translate or format your presentation.",
        "Bitten Sie Nora, Ihre Präsentation zu schreiben, zu überarbeiten, zu übersetzen oder zu formatieren.",
    ),
    ("Nora wants to:", "Nora möchte:"),
    ("Allow", "Erlauben"),
    ("Refuse", "Ablehnen"),
    ("Read the presentation", "Die Präsentation lesen"),
    ("Run {command}", "{command} ausführen"),
    ("Run {count} commands", "{count} Befehle ausführen"),
    ("Select \"{text}\"", "\"{text}\" markieren"),
    ("Type \"{text}\"", "\"{text}\" eingeben"),
    ("Look at slide {slide}", "Folie {slide} ansehen"),
    ("Save the presentation", "Die Präsentation speichern"),
    ("Undo the last change", "Die letzte Änderung rückgängig machen"),
    ("Assistant", "Assistent"),
    ("Show the Nora assistant pane (experimental)", "Nora-Bereich anzeigen (experimentell)"),
    ("Nora has nothing left to undo.", "Nora hat nichts mehr rückgängig zu machen."),
    (
        "You changed the presentation after Nora did: use the editor's Undo instead.",
        "Sie haben die Präsentation nach der Änderung durch Nora bearbeitet: Verwenden Sie stattdessen die Rückgängig-Funktion des Editors.",
    ),
];

/// Italian (formal register; the infinitive for actions and buttons).
const ITALIAN: &[(&str, &str)] = &[
    ("Nora isn't available in this version yet", "Nora non è ancora disponibile in questa versione"),
    ("New conversation", "Nuova conversazione"),
    ("Undo", "Annullare"),
    ("Undo the last change Nora made to the presentation", "Annullare l'ultima modifica apportata da Nora alla presentazione"),
    ("Ask Nora…", "Chiedere a Nora…"),
    ("Send", "Inviare"),
    ("Nora is working…", "Nora sta lavorando…"),
    (
        "Ask Nora to write, rewrite, translate or format your presentation.",
        "Chieda a Nora di scrivere, riscrivere, tradurre o formattare la sua presentazione.",
    ),
    ("Nora wants to:", "Nora desidera:"),
    ("Allow", "Consentire"),
    ("Refuse", "Rifiutare"),
    ("Read the presentation", "Leggere la presentazione"),
    ("Run {command}", "Eseguire {command}"),
    ("Run {count} commands", "Eseguire {count} comandi"),
    ("Select \"{text}\"", "Selezionare \"{text}\""),
    ("Type \"{text}\"", "Digitare \"{text}\""),
    ("Look at slide {slide}", "Consultare la diapositiva {slide}"),
    ("Save the presentation", "Salvare la presentazione"),
    ("Undo the last change", "Annullare l'ultima modifica"),
    ("Assistant", "Assistente"),
    ("Show the Nora assistant pane (experimental)", "Mostrare il pannello dell'assistente Nora (sperimentale)"),
    ("Nora has nothing left to undo.", "Nora non ha più nulla da annullare."),
    (
        "You changed the presentation after Nora did: use the editor's Undo instead.",
        "Ha modificato la presentazione dopo l'intervento di Nora: usi invece la funzione Annulla dell'editor.",
    ),
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// The `{name}` placeholders of a string, in order of appearance, sorted.
    fn placeholders(text: &str) -> Vec<String> {
        let mut names: Vec<String> = text.split('{').skip(1).filter_map(|part| part.split_once('}').map(|(name, _)| name.to_string())).collect();
        names.sort();
        names
    }

    #[test]
    fn every_language_has_every_string_and_only_those() {
        for lang in Lang::ALL.into_iter().filter(|lang| *lang != Lang::En) {
            let keys: Vec<&str> = lang.table().iter().map(|(english, _)| *english).collect();
            assert_eq!(keys, ENGLISH, "{}: the same strings as the English list, in its order", lang.code());
        }
        for english in ENGLISH {
            assert_eq!(tr(Lang::En, english), *english, "English is the key itself");
        }
    }

    #[test]
    fn every_translation_keeps_its_placeholders_and_is_clean() {
        for lang in [Lang::Fr, Lang::De, Lang::It] {
            for (english, word) in lang.table() {
                assert_eq!(placeholders(english), placeholders(word), "{}: {english:?}", lang.code());
                assert_eq!(word.trim(), *word, "{}: stray whitespace in {word:?}", lang.code());
                assert!(!word.is_empty(), "{}: {english:?} is empty", lang.code());
                assert!(!word.contains("..."), "{}: use … rather than three dots: {word:?}", lang.code());
                assert_eq!(english.ends_with('…'), word.ends_with('…'), "{}: the ellipsis of {english:?}", lang.code());
                assert_eq!(english.contains("Nora"), word.contains("Nora"), "{}: Nora keeps her name in {word:?}", lang.code());
            }
        }
        assert!(GERMAN.iter().all(|(_, word)| !word.contains('ß')), "Swiss German writes ss");
    }

    #[test]
    fn a_string_without_words_shows_in_english() {
        assert_eq!(tr(Lang::Fr, "Not a string Neoffice added"), "Not a string Neoffice added");
        assert_eq!(tr(Lang::Fr, "Send"), "Envoyer");
        assert_eq!(tr(Lang::De, "Undo"), "Rückgängig");
    }

    #[test]
    fn locale_tags_pick_the_language() {
        for (tag, lang) in [
            ("fr", Some(Lang::Fr)),
            ("fr-CH", Some(Lang::Fr)),
            ("FR_ch.UTF-8", Some(Lang::Fr)),
            ("de-CH", Some(Lang::De)),
            ("de_CH.UTF-8@euro", Some(Lang::De)),
            ("it-IT", Some(Lang::It)),
            ("en-US", Some(Lang::En)),
            ("C", Some(Lang::En)),
            ("POSIX", Some(Lang::En)),
            ("es-ES", None),
            ("", None),
        ] {
            assert_eq!(Lang::from_tag(tag), lang, "{tag:?}");
        }
        assert_eq!(Lang::system(), Lang::En, "tests run in English");
    }

    #[test]
    fn placeholders_are_filled_in_any_order() {
        assert_eq!(fmt("Run {command}", &[("command", "format.bold")]), "Run format.bold");
        assert_eq!(fmt("{count} Befehle ausführen", &[("count", "3")]), "3 Befehle ausführen");
        assert_eq!(fmt("{a} and {b}", &[("b", "2"), ("a", "1")]), "1 and 2");
        // A value is never read again for placeholders, and an unknown or unclosed one stays as written.
        assert_eq!(fmt("Type \"{text}\"", &[("text", "{text} {x")]), "Type \"{text} {x\"");
        assert_eq!(fmt("{unknown} {", &[]), "{unknown} {");
    }
}
