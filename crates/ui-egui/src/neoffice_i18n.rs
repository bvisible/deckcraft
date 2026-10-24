// //// Neoffice — added file (no upstream equivalent): the words Neoffice adds to DeckCraft, in the person's language.

//! The person's language in DeckCraft: the words Neoffice adds (the Nora pane and its switch in Preferences), and
//! DeckCraft's own interface (its ribbon, File tab, status bar and Quick Access Toolbar, [`l10n`]).
//!
//! DeckCraft has no string catalog of its own, upstream's `main` included (see `docs/localization-parity.md`): its
//! interface is English literals. Neoffice shows it in the person's language anyway: French, German, Italian or
//! Spanish when the hosting page names one (`?lang=`, else `?locale=`, which the Drive of the Suite sets to the
//! person's language), else when the system prefers one of them (in the browser build, the browser's languages),
//! English otherwise. As in WordCraft's catalogs, the strings in the code are English and are the keys; tests check that
//! every language has every one of them. When DeckCraft gets a catalog of its own, these tables join it, as
//! GridCraft's `i18n/neoffice.rs` sits beside upstream's.
//!
//! French is written for Neoffice's Swiss customers, with "vous", and reuses the wording of WordCraft's Nora pane;
//! German is Swiss Standard German ("ss", never "ß"). The interface tables follow PowerPoint's own words in each
//! language; they were made by Haiku from the English, the French read against PowerPoint's.

use std::collections::HashMap;
use std::sync::OnceLock;

mod interface_de;
mod interface_es;
mod interface_fr;
mod interface_it;
mod interface_keys;

/// A language the words Neoffice added are shown in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    /// The source language, and the one for every language without words here.
    #[default]
    En,
    Fr,
    De,
    It,
    Es,
}

impl Lang {
    /// Every language, English first.
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Fr, Lang::De, Lang::It, Lang::Es];

    /// The language's BCP 47 code.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::It => "it",
            Lang::Es => "es",
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
            "es" => Some(Lang::Es),
            _ => None,
        }
    }

    /// The first of the person's preferred languages that has words here, in their order, else English. Those are
    /// `DECKCRAFT_LANGUAGE` when it is set, then the language the hosting page names ([`set_page_language`]), then
    /// the operating system's preferred languages, or the browser's in the web build. Looked up once. Tests get
    /// English whatever the machine speaks, unless a test asks for a language (`speak_in_tests`).
    pub fn system() -> Lang {
        if cfg!(test) {
            return TEST_LANGUAGE.with(std::cell::Cell::get);
        }
        static SYSTEM: OnceLock<Lang> = OnceLock::new();
        *SYSTEM.get_or_init(|| {
            let wanted = std::env::var("DECKCRAFT_LANGUAGE").ok().and_then(|tag| Lang::from_tag(&tag));
            wanted
                .or_else(|| PAGE_LANGUAGE.get().copied().flatten())
                .or_else(|| sys_locale::get_locales().take(16).find_map(|tag| Lang::from_tag(&tag)))
                .unwrap_or_default()
        })
    }

    /// The words of the language, English first.
    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::Fr => FRENCH,
            Lang::De => GERMAN,
            Lang::It => ITALIAN,
            Lang::Es => SPANISH,
        }
    }

    /// DeckCraft's interface strings in the language, English first ([`interface_keys::KEYS`] in the same order).
    fn interface(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::Fr => interface_fr::TABLE,
            Lang::De => interface_de::TABLE,
            Lang::It => interface_it::TABLE,
            Lang::Es => interface_es::TABLE,
        }
    }
}

/// The language the hosting page names, when it names one we have words for: set once, by the browser build before
/// the interface is first drawn (`?lang=`, else `?locale=`, the person's language in the Drive of the Suite).
static PAGE_LANGUAGE: OnceLock<Option<Lang>> = OnceLock::new();

/// Takes the language the hosting page names (a tag such as `fr-CH`); the first call wins, and only before
/// [`Lang::system`] is first asked. A tag without words here leaves the system's languages to decide.
pub fn set_page_language(tag: &str) {
    let _ = PAGE_LANGUAGE.set(Lang::from_tag(tag));
}

thread_local! {
    /// The language a test speaks (English unless it asks): each test runs on a thread of its own.
    static TEST_LANGUAGE: std::cell::Cell<Lang> = const { std::cell::Cell::new(Lang::En) };
}

/// `english`, one of DeckCraft's own interface strings (a ribbon label or tooltip, a tab, a page of the File tab, the
/// status bar), in the person's language; English itself when the language has no entry for it.
pub fn l10n(english: &str) -> &str {
    l10n_in(Lang::system(), english)
}

/// `english` in `lang`, as [`l10n`] shows it.
pub fn l10n_in(lang: Lang, english: &str) -> &str {
    static INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    let index = INDEX.get_or_init(|| interface_keys::KEYS.iter().enumerate().map(|(i, key)| (*key, i)).collect());
    index.get(english).and_then(|i| lang.interface().get(*i)).map_or(english, |(_, word)| *word)
}

/// Makes the calling test speak `lang` ([`Lang::system`]) until the returned guard is dropped.
#[cfg(test)]
pub(crate) fn speak_in_tests(lang: Lang) -> impl Drop {
    struct Back;
    impl Drop for Back {
        fn drop(&mut self) {
            TEST_LANGUAGE.with(|l| l.set(Lang::En));
        }
    }
    TEST_LANGUAGE.with(|l| l.set(lang));
    Back
}

/// `english`, one of the words Neoffice added (the Nora pane), in `lang`; English itself when the language has no
/// entry for it.
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
    "Stop",
    "Stopped.",
    "Nora did not answer in time.",
    "Nora could not be reached. Check the connection and try again.",
    "Your session has expired. Sign in again.",
    "The server could not answer (error {status}). Try again in a moment.",
    "Nora's answer could not be read.",
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
    ("Stop", "Arrêter"),
    ("Stopped.", "Arrêté."),
    ("Nora did not answer in time.", "Nora n'a pas répondu à temps."),
    ("Nora could not be reached. Check the connection and try again.", "Nora n'a pas pu être jointe. Vérifiez la connexion et réessayez."),
    ("Your session has expired. Sign in again.", "Votre session a expiré. Reconnectez-vous."),
    (
        "The server could not answer (error {status}). Try again in a moment.",
        "Le serveur n'a pas pu répondre (erreur {status}). Réessayez dans un instant.",
    ),
    ("Nora's answer could not be read.", "La réponse de Nora n'a pas pu être lue."),
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
    ("Stop", "Stoppen"),
    ("Stopped.", "Gestoppt."),
    ("Nora did not answer in time.", "Nora hat nicht rechtzeitig geantwortet."),
    (
        "Nora could not be reached. Check the connection and try again.",
        "Nora war nicht erreichbar. Prüfen Sie die Verbindung und versuchen Sie es erneut.",
    ),
    ("Your session has expired. Sign in again.", "Ihre Sitzung ist abgelaufen. Bitte melden Sie sich erneut an."),
    (
        "The server could not answer (error {status}). Try again in a moment.",
        "Der Server konnte nicht antworten (Fehler {status}). Versuchen Sie es in einem Moment erneut.",
    ),
    ("Nora's answer could not be read.", "Die Antwort von Nora konnte nicht gelesen werden."),
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
    ("Stop", "Interrompere"),
    ("Stopped.", "Interrotto."),
    ("Nora did not answer in time.", "Nora non ha risposto in tempo."),
    ("Nora could not be reached. Check the connection and try again.", "Nora non è raggiungibile. Controlli la connessione e riprovi."),
    ("Your session has expired. Sign in again.", "La sessione è scaduta. Acceda di nuovo."),
    (
        "The server could not answer (error {status}). Try again in a moment.",
        "Il server non ha potuto rispondere (errore {status}). Riprovi tra poco.",
    ),
    ("Nora's answer could not be read.", "Non è stato possibile leggere la risposta di Nora."),
];

/// Spanish (Spain, "usted"), made by Haiku from the English.
const SPANISH: &[(&str, &str)] = &[
    ("Nora isn't available in this version yet", "Nora todavía no está disponible en esta versión"),
    ("New conversation", "Nueva conversación"),
    ("Undo", "Deshacer"),
    ("Undo the last change Nora made to the presentation", "Deshacer el último cambio que Nora hizo en la presentación"),
    ("Ask Nora…", "Pregunte a Nora…"),
    ("Send", "Enviar"),
    ("Nora is working…", "Nora está trabajando…"),
    (
        "Ask Nora to write, rewrite, translate or format your presentation.",
        "Pida a Nora que escriba, reescriba, traduzca o dé formato a su presentación.",
    ),
    ("Nora wants to:", "Nora quiere:"),
    ("Allow", "Permitir"),
    ("Refuse", "Rechazar"),
    ("Read the presentation", "Leer la presentación"),
    ("Run {command}", "Ejecutar {command}"),
    ("Run {count} commands", "Ejecutar {count} comandos"),
    ("Select \"{text}\"", "Seleccionar \"{text}\""),
    ("Type \"{text}\"", "Escribir \"{text}\""),
    ("Look at slide {slide}", "Ver la diapositiva {slide}"),
    ("Save the presentation", "Guardar la presentación"),
    ("Undo the last change", "Deshacer el último cambio"),
    ("Assistant", "Asistente"),
    ("Show the Nora assistant pane (experimental)", "Mostrar el panel del asistente Nora (experimental)"),
    ("Nora has nothing left to undo.", "Nora no tiene nada más que deshacer."),
    (
        "You changed the presentation after Nora did: use the editor's Undo instead.",
        "Modificó la presentación después de Nora: use en su lugar la opción Deshacer del editor.",
    ),
    ("Stop", "Detener"),
    ("Stopped.", "Detenido."),
    ("Nora did not answer in time.", "Nora no respondió a tiempo."),
    ("Nora could not be reached. Check the connection and try again.", "No se pudo contactar con Nora. Compruebe la conexión e inténtelo de nuevo."),
    ("Your session has expired. Sign in again.", "Su sesión ha caducado. Inicie sesión de nuevo."),
    (
        "The server could not answer (error {status}). Try again in a moment.",
        "El servidor no pudo responder (error {status}). Inténtelo de nuevo en unos instantes.",
    ),
    ("Nora's answer could not be read.", "No se pudo leer la respuesta de Nora."),
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
        for lang in [Lang::Fr, Lang::De, Lang::It, Lang::Es] {
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
            ("es-ES", Some(Lang::Es)),
            ("pt-BR", None),
            ("", None),
        ] {
            assert_eq!(Lang::from_tag(tag), lang, "{tag:?}");
        }
        assert_eq!(Lang::system(), Lang::En, "tests run in English");
    }

    #[test]
    fn every_interface_table_has_every_key_in_its_order() {
        let mut seen = std::collections::HashSet::new();
        for key in interface_keys::KEYS {
            assert!(seen.insert(*key), "{key:?} is listed twice");
        }
        for lang in Lang::ALL.into_iter().filter(|lang| *lang != Lang::En) {
            let keys: Vec<&str> = lang.interface().iter().map(|(english, _)| *english).collect();
            assert_eq!(keys, interface_keys::KEYS, "{}: the interface keys, in their order", lang.code());
        }
    }

    #[test]
    fn every_interface_translation_keeps_its_lines_placeholders_and_shortcuts() {
        for lang in [Lang::Fr, Lang::De, Lang::It, Lang::Es] {
            for (english, word) in lang.interface() {
                let what = format!("{}: {english:?} -> {word:?}", lang.code());
                assert!(!word.trim().is_empty(), "{what}: empty");
                assert_eq!(english.matches('\n').count(), word.matches('\n').count(), "{what}: a two-line label stays on two lines");
                assert_eq!(placeholders(english), placeholders(word), "{what}");
                assert_eq!(english.ends_with('…'), word.ends_with('…'), "{what}: the ellipsis");
                assert!(!word.contains("..."), "{what}: … rather than three dots");
                assert_eq!(english.ends_with(' '), word.ends_with(' '), "{what}: the trailing space");
                assert_eq!(english.starts_with(' '), word.starts_with(' '), "{what}: the leading space");
                // A shortcut in parentheses is the keyboard's, in every language.
                if let Some(start) = english.rfind(" (")
                    && english.ends_with(')')
                    && english[start..].chars().any(|c| matches!(c, '⌘' | '⇧' | '⌥' | '+') || english[start..].contains("Esc"))
                {
                    assert!(word.ends_with(&english[start + 1..]), "{what}: the shortcut");
                }
            }
        }
        assert!(Lang::De.interface().iter().all(|(_, word)| !word.contains('ß')), "Swiss German writes ss");
    }

    #[test]
    fn the_tabs_the_file_tab_and_the_status_bar_have_words() {
        let tabs = ["Home", "Insert", "Draw", "Design", "Transitions", "Animations", "Slide Show", "Record", "Review", "View", "File"];
        let contextual = ["Shape Format", "Picture Format", "Table Design", "Layout", "SmartArt Design", "Chart Design", "Playback", "Slide Master"];
        let pages = crate::backstage::Page::ALL.map(crate::backstage::Page::label);
        let side = ["Save", "Close", "Options"];
        let status =
            ["Slide {n} of {count}", "Notes", "Comments", "Zoom In", "Zoom Out", "Normal", "Slide Sorter", "Reading View", "Click to add notes"];
        for key in tabs.iter().chain(&contextual).chain(&pages).chain(&side).chain(&status) {
            assert!(interface_keys::KEYS.contains(key), "{key:?} has no words in the other languages");
        }
        assert_eq!(l10n_in(Lang::Fr, "Home"), "Accueil");
        assert_eq!(l10n_in(Lang::Fr, "Insert"), "Insertion");
        assert_eq!(l10n_in(Lang::Fr, "Save As"), "Enregistrer sous");
        assert_eq!(l10n_in(Lang::De, "Slide Show"), "Bildschirmpräsentation");
        assert_eq!(l10n_in(Lang::En, "Home"), "Home");
        assert_eq!(l10n_in(Lang::Fr, "Not a string of the interface"), "Not a string of the interface");
    }

    #[test]
    fn the_person_s_language_is_the_one_l10n_speaks() {
        assert_eq!(l10n("Home"), "Home", "tests speak English unless they ask");
        {
            let _french = speak_in_tests(Lang::Fr);
            assert_eq!(Lang::system(), Lang::Fr);
            assert_eq!(l10n("Home"), "Accueil");
            assert_eq!(fmt(l10n("Slide {n} of {count}"), &[("n", "2"), ("count", "6")]), "Diapositive 2 sur 6");
            // The Nora pane follows the same language.
            assert_eq!(tr(Lang::system(), "Send"), "Envoyer");
        }
        assert_eq!(l10n("Home"), "Home", "back to English when the guard goes");
    }

    /// The ribbon and the File tab drawn in French: the buttons and the side menu are named in French (the names screen
    /// readers and these tests read are the words shown).
    #[test]
    fn the_ribbon_and_the_file_tab_speak_french() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let _french = speak_in_tests(Lang::Fr);
        let app = crate::SlideApp::new(deckcraft_engine::Session::with_new(), crate::Services::default());
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
            |ui, app: &mut crate::SlideApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            app,
        );
        h.run_steps(3);
        assert!(!h.query_all_by_label("Nouvelle diapositive").collect::<Vec<_>>().is_empty(), "the Home tab's New Slide");
        h.get_by_label("Fichier").click();
        h.run_steps(3);
        assert!(h.state().backstage.is_some(), "Fichier opens the File tab");
        for page in ["Informations", "Enregistrer sous", "Imprimer", "Exporter"] {
            assert!(!h.query_all_by_label(page).collect::<Vec<_>>().is_empty(), "{page} in the side menu");
        }
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
