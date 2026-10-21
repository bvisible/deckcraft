# Neoffice fork markers

Map of **our** divergence from upstream (`storytold/deckcraft`) in this fork (`bvisible/deckcraft`, branch
`version-15`). Everything that can carry a comment carries a `//// Neoffice — …` marker in place:
`// //// Neoffice — …` in Rust (a bare `////` is an error for clippy), `# //// Neoffice — …` in TOML
and YAML, `<!-- //// Neoffice — … -->` in Markdown. This file holds the base of the divergence and
what cannot carry a comment.

At the next upstream merge, `grep -rn "////"` in the source plus this file give the complete
picture of what is ours and why.

- **Upstream**: `storytold/deckcraft`, branch `main`.
- **Base of the divergence**: `688c7ef592c901ca4541c25e210c1b302e4dc066`, the tag `v0.6.1`, where `version-15` was cut on 2026-10-13.
  Move it at each upstream merge. The check works it out itself (the common ancestor with
  upstream's `main`), so this line is for the reader.

## What the fork adds

Three workflow files and the features below, all marked in place. The workflow files are new files that carry
their own `//// Neoffice — added file` header.

- `.github/workflows/neoffice-web.yml`: builds the browser version on a GitHub runner, started by
  hand, with the packaging script upstream's own release job runs. Upstream's workflows are
  disabled on this fork, because they need upstream's signing secrets.
- `.github/workflows/neoffice-markers.yml`: fails a push to `version-15` when a change to
  upstream's code since the common ancestor carries no marker.
- `.github/workflows/neoffice-ci.yml`: runs upstream's own quality gate, `cargo xtask ci`, on a
  GitHub runner (by hand, or on a push that touches code), because the maintainer's Mac has too
  little disk to run it for every change.
- **The host bridge of the browser build** (new file `apps/deckcraft-web/src/host.rs`; marked changes
  in `apps/deckcraft-web/src/web.rs`, `apps/deckcraft-web/src/main.rs`, the web-sys features of
  `apps/deckcraft-web/Cargo.toml`, `crates/ui-egui/src/lib.rs` and `crates/engine/src/lib.rs`,
  `crates/engine/src/cmd/file.rs`, with a test in `crates/engine/src/tests.rs`): the `postMessage`
  contract of a page that frames the editor (`?host=parent`, same origin), the one upstream's
  GridCraft and PdfCraft already use, so one host page (the Drive of the Suite, NeoDesk's window)
  drives every Craft editor: `deckcraft:ready`, `deckcraft:open` (a file to edit), `deckcraft:save`
  (every file the editor would download), `deckcraft:dirty` and `deckcraft:failed`, plus
  `deckcraft:saved` (`ok: false` marks the presentation unsaved again and says why: ours, upstream's
  bridges have no answer to a save) and `?author=` (the name that signs comments, `Prefs::author`).
  Upstream's DeckCraft has no such bridge, neither in v0.6.1 nor in `main`. Without `?host=parent`
  the editor behaves as before. The editor side of it is two small additions to upstream's code:
  `Services::host_save`, a hook that Save and Save As use in place of `download` (in `save_as_dialog`),
  and `DocState::opened_from`, the name of the file a presentation was opened from
  (`file.openBytes` sets it).
  - **Save and Save As write PowerPoint files.** A presentation opened from `x.pptx`, `x.potx` or
    `x.ppsx` goes back to the page as a PowerPoint file under the same name; one opened from
    `x.deckcraft` stays a `.deckcraft`; one made in the editor is `<title>.pptx`. (Without the bridge
    the web build downloads `<title>.deckcraft`, DeckCraft's own zip, whatever was opened.) DeckCraft
    writes a `.potx` or `.ppsx` as an ordinary presentation package, so PowerPoint may not take such a
    file for a template or a show.
  - In host mode the editor starts on a blank presentation (what `?blank` opens), not on the sample
    deck, and the blank one makes way when a file opened from bytes (the page's) is there.
  - The interface is English only (DeckCraft has no string catalog yet, see
    `docs/localization-parity.md`), so the status line "Couldn't save {name}: {error}" has no French.
    Its French, for when there is a catalog: "Impossible d'enregistrer {name} : {error}".
  - Drop the bridge when upstream ships its own for DeckCraft, keeping `deckcraft:saved`, the
    PowerPoint save and the blank start.

- **No Discord link in the About dialog nor on the start screen** (`crates/ui-egui/src/dialogs.rs`, two marked places):
  the links to upstream's community are not shown, for everyone who uses this build. The MCP server's instructions and
  the `app.links` command list the project's addresses for agents, and stay as upstream has them.

- **A hosting page opens the presentations** (`Services::host_owns_documents`, set by `apps/deckcraft-web/src/web.rs` after
  the editor's own blank start, so only when a page frames the editor with `?host=parent`): the editor does not bring another presentation in beside
  the page's, close it or show its start screen. `file.new`, `file.close`, `file.open`, `file.openBytes`,
  `app.openDialog` and `app.home` are refused with a status message (`run` in `crates/ui-egui/src/lib.rs`), the start
  screen says that the page gives the presentation (`dialogs.rs`), the title bar has no Home button (`ribbon.rs`), and
  a presentation dropped on the editor is not taken (`opens_as_presentation`, asked by `apps/deckcraft-web/src/web.rs`);
  the file the page hands over opens through the inbox, which does not come through `run`. Three tests in
  `crates/ui-egui/src/lib.rs`. The message is English only, like the rest of the interface. Every other build behaves
  as before.

- **The account button of the title bar** (new file `crates/ui-egui/src/account.rs`, with its tests; one marked line in
  `lib.rs` and one in `ribbon.rs`'s `title_bar`): at the right end of the title bar, the button WordCraft has there, so
  the four editors show the person the same way: a round badge in the accent colour, a square of the bar's control
  height (26, the Present button's), with up to two initials (the first letters of the words of `Prefs::author`, the
  name a hosting page passes as `?author=`), and the name, nothing else, as the hover text. The initials take, of
  white and the theme's dark, the one that reads better on the accent, at least 4.5:1 (a test checks it): the title
  bar's own colour on the dark theme (5:1; white, the theme's colour for text on the accent, reads at 3:1 there),
  black on the light theme (5.4:1; white 3.9:1, and none of the theme's greys reaches 4.5:1 on its orange). It shows
  only when a name is known: not while the name is blank or the engine's placeholder ("Presenter"). No new text.

- **The Nora assistant pane** (new files `crates/ui-egui/src/nora_pane.rs` and `crates/ui-egui/src/neoffice_i18n.rs`, with
  their tests, `apps/deckcraft-web/src/web/nora.rs` and `apps/deckcraft-web/src/nora_origin.rs`; marked changes in the workspace `Cargo.toml`,
  `crates/ui-egui/Cargo.toml`, `crates/ui-egui/src/lib.rs` (the modules, `Services::nora_http` and `nora_inbox`,
  `UiState::nora_pane`, the `ui.nora` command, the pane in the layout, `SlideApp::nora`), `crates/ui-egui/src/dialogs.rs`
  (Preferences › Assistant), the web-sys features of `apps/deckcraft-web/Cargo.toml`, `apps/deckcraft-web/src/web.rs` and
  `apps/deckcraft-web/src/main.rs`): Neoffice's assistant, the chat at the right of the window that NeoWrite (WordCraft)
  has, so that the same Nora works in every editor. It is built on `craft-nora` and `craft-nora-ui` from Neoffice's
  craft-libs fork, at the revision WordCraft uses; to follow a newer one, move the revision in `Cargo.toml`.
  - **When it shows.** Off by default. Preferences › Assistant, or `ui.nora` (`on`, or WordCraft's `value`; without
    either it toggles; listed as "Nora" in the command palette), turns it on, and the desktop app keeps the setting
    (`noraPane` in `ui.json`). The browser build keeps no setting, so it opens the pane itself when the page asks for Nora
    (`?nora`, read by name with `URLSearchParams`) or makes Nora's calls itself (`window.deckcraftNoraCall(path, body)` on
    the page or its same-origin parent, as Neodesk does). Outermost at the right, in every view, not on the start screen
    nor in a slide show.
  - **Where the calls go** (`nora_origin.rs`, tested natively; the rule is common to Neoffice's editors): `?nora` is the
    page's own origin; `?nora=<origin>` is taken only when it is the page's own origin, or when the page and that origin
    are both loopback (`127.0.0.1`, `localhost`, `[::1]`, for the workshop's stand-in server); any other origin only from
    `window.deckcraftNora.origin`, which the hosting page sets. The page's credentials (a Frappe page's CSRF token,
    `window.deckcraftNora.headers`) go only with calls to its own origin. A link can't send the person's conversation,
    what Nora reads of the presentation or the pictures of its slides to a server it names.
  - **The connection is the host's.** The pane never opens one: the host makes the calls with its own session
    (`Services::nora_http`) and delivers the answers (`Services::nora_inbox`). Without one the pane says that Nora isn't
    available and offers no text box.
  - **Nora's actions are DeckCraft's commands,** played through `SlideApp::run`, the surface the MCP tools use. Observe:
    every slide's title and text in order (`Slide 2 (current): …`, a table row by row, the notes), the slide count, the
    current slide, the words, the selection and its formatting, the text cut at 50,000 characters. Execute and Batch:
    the commands by id, except those of files and of the application (`file.`, `app.`), of the interface (`ui.`, every
    command of `UI_COMMANDS`: views, panes, zoom, ribbon, dark mode), the slide show (`show.fromStart`,
    `show.fromCurrent`), the switch to another presentation (`window.`), the person's undo history (`edit.undo`,
    `edit.redo`: a batch that undoes would take back their work, and the pane's Undo theirs as Nora's), their clipboard
    (`edit.copy`, `edit.cut`, `edit.paste` with `edit.pasteText`: every open presentation shares it) and media playback, which decodes a file's audio or video (`media.play`, `media.seek`,
    `media.toggle`, `media.posterFrame`), and except any command with a parameter naming a file (`path`, `file`,
    `filename`, `folder`, `directory`…, at any depth; `dir` is a direction in DeckCraft). SelectText: the n-th occurrence
    on the current slide (its shapes in order, a table cell by cell, then its notes), through `text.edit` and
    `text.select`. TypeText: into the text being edited, else into the one selected shape that holds text (its text
    replaced, as typing does), else into the slide's empty body or content placeholder (one that holds text, not a picture,
    chart or table), else into a new text box below
    the title, at the top of the tallest strip of the slide nothing covers (its shapes, and the graphics its layout and
    master draw), across the middle where no strip has room (`insert.textBox`, then `text.insert`), with AutoCorrect and
    smart quotes off while Nora types. RenderPage: `file.render` of that slide number. Save: `file.save`, so a
    presentation without a file goes through the app's Save As (the hosting page, a dialog, a download). Nora's actions
    wait, in their order, while the pointer drags something on the slide or a slide in the thumbnails or the sorter.
  - **A failed action leaves the presentation as it was:** its document, selection and undo history (the redo steps
    included) go back to a snapshot taken before it. **Undo** (the pane's button) takes back Nora's last change whole,
    counting the undo steps it left even when the history is full, and refuses when the presentation changed after it
    (it would otherwise take back the person's own work). An action that changed nothing (a read, a move to another
    slide) answers `"changed": false` and leaves nothing to undo, which newer craft-nora revisions read; a batch's list
    of results can't carry the mark.
  - **The words** (the pane, its Preferences switch, Undo's two refusals) are in French, German or Italian when the
    system or the browser prefers one of them (`sys-locale`, `DECKCRAFT_LANGUAGE` first), English otherwise. DeckCraft has
    no catalog, so they live in Neoffice's own table, `neoffice_i18n.rs`; French reuses the wording of WordCraft's Nora
    pane. The rest of the Preferences dialog stays English. Errors that come from the engine are its own, in English.

- **The browser build reads its address's flags by name** (`apps/deckcraft-web/src/web.rs`, `has_param`, with
  `URLSearchParams` as `host.rs` reads its own): upstream looked for `webgl`, `blank` and `show` anywhere in the address's
  text, so a host name (`?nora=https://showroom…`) or a name the page passes (`?author=`) started the slide show over the
  editor. Drop it when upstream reads them by name.

- **The interface's Latin font comes before its Japanese one** (`crates/ui-egui/src/theme.rs`, `install_fonts`, with a
  test): Inter when craft-fonts has it, else egui's Ubuntu Light, then BIZ UDPGothic and Noto Sans Arabic for their
  scripts. Upstream put the Japanese face first when craft-fonts has no Inter (this Mac's checkout has none); its dashes
  (U+2013, U+2014) are hairlines 0.05 em thick that vanish at interface sizes, so the em dash didn't show in Nora's pane
  nor in the Outline view. Builds with Inter, and builds without craft-fonts, looked as before.

- **Our product's name in our builds** (new file `crates/ui-egui/src/brand.rs`): `brand::name()` is
  `NEOFFICE_PRODUCT_NAME` when the build sets it (only the build step of `neoffice-web.yml`: `NeoPrés`), upstream's
  name otherwise, so upstream's builds and every test run are unchanged. DeckCraft has no string catalog, so the places
  that show the app's name call `brand::name()`, `brand::about()` or `brand::label()`: the title bar without a
  presentation (`ribbon.rs`), the About dialog and its title, the start screen, the quit dialog's title (`dialogs.rs`),
  the Help menu and the command palette (`menus.rs`), the Presenter View's title (`show.rs`), a status message
  (`lib.rs`) and the page that failed to start (`apps/deckcraft-web/src/web.rs`); `crates/engine/src/cmd/file.rs`,
  below the interface, applies the same rule to its "not a presentation … can read" error. The page title and the
  loading text are ours in `apps/deckcraft-web/index.html`. The `DeckCraft` theme, storage folders, `deckcraft:*`
  messages, `ai.storyteller.*`, the file format's generator, the licence line and the native app keep upstream's name.

## Files that cannot carry a comment

- `Cargo.lock`: the Nora pane's dependencies, `craft-nora` and `craft-nora-ui` from `bvisible/craft-libs` (git, revision
  `adbcef6c286b6bcd264cc91fbf50dc3f42d60e5a`, the one WordCraft uses) and `sys-locale` 0.3.2 (the person's preferred
  languages). Nothing else moved. At an upstream merge, take upstream's lock and let cargo add these back.
