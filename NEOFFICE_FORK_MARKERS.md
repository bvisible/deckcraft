# Neoffice fork markers

Map of **our** divergence from upstream (`storytold/deckcraft`) in this fork (`bvisible/deckcraft`, branch
`version-15`). Everything that can carry a comment carries a `//// Neoffice — …` marker in place:
`// //// Neoffice — …` in Rust (a bare `////` is an error for clippy), `# //// Neoffice — …` in TOML
and YAML, `<!-- //// Neoffice — … -->` in Markdown. This file holds the base of the divergence and
what cannot carry a comment.

At the next upstream merge, `grep -rn "////"` in the source plus this file give the complete
picture of what is ours and why.

- **Upstream**: `storytold/deckcraft`, branch `main`.
- **Base of the divergence**: `b47195feec0f0fe4220d8943ad1c8add5dad8072`, the tag `v0.10.0`, merged on 2026-10-24
  (before: `fe510bdac68118f2c9a714e69d0ff572e3f9a0c7`, the tag `v0.9.0`, merged on 2026-10-22; and
  `688c7ef592c901ca4541c25e210c1b302e4dc066`, the tag `v0.6.1`, where `version-15` was cut on 2026-10-13).
  Move it at each upstream merge. The check works it out itself (the common ancestor with
  upstream's `main`), so this line is for the reader.

## What the fork adds

Four workflow files and the features below, all marked in place. The workflow files are new files that carry
their own `//// Neoffice — added file` header.

- `.github/workflows/neoffice-web.yml`: builds the browser version on a GitHub runner, started by
  hand, with the packaging script upstream's own release job runs. Upstream's workflows are
  disabled on this fork, because they need upstream's signing secrets.
- `.github/workflows/neoffice-markers.yml`: fails a push to `version-15` when a change to
  upstream's code since the common ancestor carries no marker.
- `.github/workflows/neoffice-ci.yml`: runs upstream's own quality gate, `cargo xtask ci`, on a
  GitHub runner (by hand, or on a push that touches code), because the maintainer's Mac has too
  little disk to run it for every change.
- `.github/workflows/neoffice-nora-catalogue.yml`: exports, by hand at the revision the Suite pins, the editor's
  command list (`list_commands` of `deckcraft-cli mcp`) and the Nora pane's verdict on each, for the catalogue Nora
  reads on the Neoffice server.
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
    `x.ppsx` goes back to the page as a PowerPoint file under the same name, a template as a template and
    a show as a show (the engine writes their own content types since v0.8); one opened from
    `x.deckcraft` stays a `.deckcraft`; one made in the editor is `<title>.pptx`. (Without the bridge
    the web build downloads `<title>.deckcraft`, DeckCraft's own zip, whatever was opened.) In host mode
    the File tab's Save As offers the page's format only (`save_as_types` in `backstage.rs`): the page keeps one
    file, and another type would reach it under another name, a second document beside the one the editor holds
    and marks saved. Copies in other formats are the Export page's.
  - **A file the page sent that can't be opened is said back** (`deckcraft:failed {error}`, the message the contract
    has for an editor that couldn't start): no bytes to read, a name that isn't a presentation's (it is not inserted
    into the presentation the editor has), or a file the editor can't read. The editor stays on what it had.
  - In host mode the editor starts on a blank presentation (what `?blank` opens), not on the sample
    deck, and the blank one makes way when a file opened from bytes (the page's) is there.
  - The page's file (`deckcraft:open`) waits in the bridge and opens at the start of the next frame through
    `SlideApp::open_or_insert_bytes`, as a file dropped on the window does. Since v0.8 upstream's `Inbox` carries
    `InboxEvent`s matched to a request the editor began (`begin_external_file`), which a message from the page
    has none of.
  - The interface is English only (DeckCraft has no string catalog yet, see
    `docs/localization-parity.md`), so the status line "Couldn't save {name}: {error}" has no French.
    Its French, for when there is a catalog: "Impossible d'enregistrer {name} : {error}".
  - Drop the bridge when upstream ships its own for DeckCraft, keeping `deckcraft:saved`, the
    PowerPoint save and the blank start.

- **No Discord link in the About dialog nor in the File tab** (`crates/ui-egui/src/dialogs.rs`, the About dialog;
  `crates/ui-egui/src/backstage.rs`, its Home page, which is also the start screen, and its Account page): the links to
  upstream's community are not shown, for everyone who uses this build. The MCP server's instructions and
  the `app.links` command list the project's addresses for agents, and stay as upstream has them.

- **A hosting page opens the presentations** (`Services::host_owns_documents`, set by `apps/deckcraft-web/src/web.rs` after
  the editor's own blank start, so only when a page frames the editor with `?host=parent`): the editor does not bring another presentation in beside
  the page's, close it or show its start screen. `file.new`, `file.close`, `file.open`, `file.openBytes`, `file.adopt`,
  `file.recovery.open`, `app.openDialog`, `app.home` and `app.closePresentation` (`hidden_from_a_host`), and
  `app.backstage` asked for Home, New or Open (`backstage_leaves_the_page`), are refused with a status message (`run` in
  `crates/ui-egui/src/lib.rs`); so is `close_presentation`, which Cmd+W, the File menu and the File tab call without
  `run`, and so are the engine's own shortcuts for them (Cmd+N, and Cmd+W while a freeform shape is drawn), which
  `Session::key` runs without `run` (`host_keeps_the_key`). The File tab (upstream's backstage since v0.8, `crates/ui-egui/src/backstage.rs`) opens on Info, never on Home,
  New or Open (`open_backstage`), and its side menu has no Home, New, Open nor Close (`entries_for`); its Home page,
  the start screen while the page's file has not arrived, only says that the page gives the presentation, and Save As
  says Save, not Download. The Quick Access Toolbar (upstream's since v0.8, `qat.rs`, which has Home by default) leaves
  out Home, New and Open, in the bar and in its Customize menu, without changing the person's toolbar. The command
  palette does not list them (`menus.rs`). A presentation dropped on the editor is not taken (`opens_as_presentation`,
  asked by `apps/deckcraft-web/src/web.rs`, which cancels the request it began). The file the page hands over opens
  without `run` (see the bridge). Six tests in `crates/ui-egui/src/lib.rs` and one in `apps/deckcraft-web/src/host.rs`.
  The words are English only, like the rest of the interface. Every other build behaves as before.

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
  craft-libs fork, at the revision `Cargo.toml` names (not always WordCraft's); to follow a newer one, move it there.
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
    `media.toggle`, `media.posterFrame`) or stops it on the person's screen (`media.pause`, `media.stop`, `media.gifPlay`), and except any command with a parameter naming a file (`path`, `file`,
    `filename`, `folder`, `directory`…, at any depth; `dir` is a direction in DeckCraft). The prefixes also refuse
    upstream's `edit.pasteSpecial` (the clipboard) and `view.notesMaster` (under the interface's `view.notes`); the Slide and
    Handout Masters stay Nora's. SelectText: the n-th occurrence
    on the current slide (its shapes in order, a table cell by cell, then its notes), through `text.edit` and
    `text.select`. TypeText: into the text being edited, else into the one selected shape that holds text (its text
    replaced, as typing does), else into the slide's empty body or content placeholder (one that holds text, not a picture,
    chart or table), else into a new text box below
    the title, at the top of the tallest strip of the slide nothing covers (its shapes, and the graphics its layout and
    master draw), across the middle where no strip has room (`insert.textBox`, then `text.insert`), with AutoCorrect and
    smart quotes off while Nora types. RenderPage: `file.render` of that slide number. Save: `file.save`, so a
    presentation without a file goes through the app's Save As (the hosting page, a dialog, a download). Nora's actions
    wait, in their order, while the pointer drags something on the slide (moving, resizing, or a tool's drag: drawing a
    shape, a text box, ink or a freeform, selecting text, a guide), a slide in the thumbnails or the sorter, a whole
    section in the sorter or an object's row in the Selection pane (the last two are upstream's since v0.8), and, when they
    change the presentation or open something on the person's screen (Save, Print), while the File tab (or the start
    screen) or a slide show covers the slides; her reads (the presentation, a slide's picture, its PDF or file for an
    e-mail) go on. The conversation goes on meanwhile, its clock and its reads (`nora_pane::keep_up`, called by `lib.rs`
    when the pane is on but not drawn), and the actions play in their order once the person is back, except one that
    waited more than a minute (Nora has asked again by then) or for another presentation than the active one: Nora is
    told it was not done.
  - **A failed action leaves the presentation as it was:** its document, selection and undo history (the redo steps
    included) go back to a snapshot taken before it. **Undo** (the pane's button) takes back Nora's last change whole,
    counting the undo steps it left even when the history is full, and refuses when the presentation changed after it
    (it would otherwise take back the person's own work). An action that changed nothing (a read, a move to another
    slide) answers `"changed": false` and leaves nothing to undo, which newer craft-nora revisions read; a batch's list
    of results can't carry the mark.
  - **A tab left in the background** runs no frame, so nothing Nora asks meanwhile is played: an action that waited in
    the page is refused when the page comes back, never played late, and so is the rest of that turn (craft-nora). For
    that the pane ticks its session first (`drive`), then hands it the answers the host delivered (craft-libs ed1cb40,
    1f01bcc, as GridCraft does). Once the turn is over the pane says the tab was in the background and offers Resume
    (`resume_words`), which sends the request again on the click only. DeckCraft carries the answers to Nora's cards, so
    `cards_unanswered` stays unset.
  - **The words** (the pane, its Preferences switch, Undo's two refusals, the resume sentence and button) are in French, German, Italian or Spanish when the
    hosting page names one, or the system or the browser prefers one of them (`DECKCRAFT_LANGUAGE` first, then the page's
    `?lang=` or `?locale=`, then `sys-locale`), English otherwise: the language of the interface (see below). DeckCraft has
    no catalog, so they live in Neoffice's own table, `neoffice_i18n.rs`; French reuses the wording of WordCraft's Nora
    pane. The rest of the Preferences dialog stays English. Errors that come from the engine are its own, in English.
    Since v0.8 the Preferences dialog has Cancel, which puts back what it opened with: the Nora switch is in that snapshot
    (`preferences_snapshot`, `restore_preferences`).

- **The interface in the person's language** (from 2026-10-24; `crates/ui-egui/src/neoffice_i18n.rs`, `l10n`, and its
  tables, new files under `crates/ui-egui/src/neoffice_i18n/`: `interface_keys.rs` and one table each for French,
  German, Italian and Spanish, made by Haiku from the English with PowerPoint's own words): DeckCraft has no string
  catalog, upstream's `main` included (`docs/localization-parity.md`), so its ribbon (tabs, buttons, menus, tooltips),
  File tab (side menu, pages), status bar and Quick Access Toolbar are drawn through `l10n(english)`, which returns the
  English itself when the language has no entry. The calls are marked `//// Neoffice — in the person's language
  (l10n)` where they are made: inside the widgets of `widgets.rs` (`big_button_ex`, `small_button_ex`, `check_item`,
  `icon_toggle`, `color_split`, `section`, `drop_button`), the side-menu and page helpers of `backstage.rs`, the
  status bar's helpers, and around the literals of `ribbon.rs`, `backstage.rs`, `status.rs` and `qat.rs` (buttons,
  labels, tooltips, menus, check boxes, hints), and, from the second pass the same day, of the dialogs (`dialogs.rs`:
  titles, `field`, `check`, `text_row`, buttons, save prompts), the panes (`panes.rs`, `selpane.rs`, `fillui.rs`), the
  slide show (`show.rs`), the canvas, thumbnail and sorter menus (`canvas.rs`, `menus.rs`, `thumbs.rs`, `sorter.rs`),
  the command palette (`menus.rs::palette_items`, which still finds a command by its English name and its id), `opening.rs`, `doc_window.rs`, the toasts and status messages
  (`lib.rs::fresh_status` when shown, `notify`, through `l10n_owned`; `status` keeps the English that `inspect` reports) and the Print page (its PDF sentence, the range example, the
  paper names), with the choices of their lists and combo boxes, the items of the menus built from a list (the canvas's Cut, Copy, Paste, the ribbon's drop-downs, the transition sounds, Merge Shapes, the artistic effects) and the colour picker's words. Ids stay English (`app.ui.tab`, QAT entries, `Page::label` as keys).
  The language: `DECKCRAFT_LANGUAGE`, then the one the hosting page names (`?lang=`, else `?locale=`, which the Drive
  of the Suite sets to the person's language, read by `apps/deckcraft-web/src/web.rs`), then the system's or the
  browser's; English otherwise. Tests speak English unless they ask (`speak_in_tests`). Still English: the engine's
  messages, and words made at run time from data (a theme's or a layout's name).
  Drop the tables when upstream ships a catalog; its entries win, ours fill its gaps, as GridCraft's `i18n/neoffice.rs`.

- **A ribbon too narrow for a tab folds its small buttons to icons** (from 2026-10-24; `widgets.rs`, `set_folded`,
  `small_button_drawn`, `small_button_tip`, and `ribbon.rs::show`): upstream cuts off the groups at the right when a tab
  is wider than the window, which came sooner with the longer words of French or German. `ribbon::show` measures each
  tab unfolded (per tab and language) and, while that width does not fit, draws its small buttons with their icon only,
  as PowerPoint does on a narrow window; the label stays the button's accessible name and becomes its tooltip, unless
  the caller gives its own (`small_button_tip`). Only the ribbon's layer folds; the menus its buttons open keep their
  words. A tab still too wide folded scrolls sideways (`ScrollArea::horizontal`) rather than cut off its last groups. Drop it when upstream collapses its groups.

- **The browser build reads its address's flags by name** (`apps/deckcraft-web/src/web.rs`, `has_param`, with
  `URLSearchParams` as `host.rs` reads its own): upstream looked for `webgl`, `blank` and `show` anywhere in the address's
  text, so a host name (`?nora=https://showroom…`) or a name the page passes (`?author=`) started the slide show over the
  editor. Drop it when upstream reads them by name.

- **The interface's Latin font comes before its Japanese one** (`crates/ui-egui/src/theme.rs`, `font_definitions`, which
  `install_fonts` uses since v0.8, with a test): Inter when craft-fonts has it, else egui's Ubuntu Light, then BIZ
  UDPGothic and Noto Sans Arabic for their scripts. Upstream put the Japanese face first when craft-fonts has no Inter
  (this Mac's checkout has none); its dashes (U+2013, U+2014) are hairlines 0.05 em thick that vanish at interface sizes,
  so the em dash didn't show in Nora's pane nor in the Outline view. Builds with Inter, and builds without craft-fonts,
  looked as before. Upstream's v0.8 cures the same dashes another way (the Japanese face unhinted, with its own test);
  both stay: ours keeps Latin text in a Latin face.

- **`slide.hide` and `slide.layout` refuse an index that is no slide** (`crates/engine/src/cmd/slide.rs`, `existing_slide`,
  with a test, from 2026-10-24): a negative or fractional `index` fell back to the selected slides, one past the last
  slide did nothing but answered as if done (an empty undo step). Upstream's v0.10.0 refuses it in `slide.delete`,
  `slide.duplicate` and `slide.move` (`opt_usize_param`) but not in these two. Drop when upstream refuses it too.

- **The browser build has the presentation fonts** (`.github/workflows/neoffice-web.yml`, `crates/fonts/build.rs`, from
  2026-10-22): the web build checks out craft-fonts at the commit upstream's CI pins (`43913056`) and sets
  `CRAFT_FONTS_DIR` (with `CRAFT_FONTS_REQUIRED`), as upstream's release does; before, it drew every presentation in
  egui's Ubuntu Light, with no bold. What a wasm build embeds stays upstream's choice (Inter Regular and Bold, BIZ
  UDPGothic Regular), plus Inter Italic (one marked line in `build.rs`), so italics are not drawn upright. Adobe's
  families (Source Sans 3…) stay out, as upstream's AGENTS.md §1.1 has it: a file asking for Source Sans 3 draws in
  Inter (`fontdb::substitutes`). The zip carries the fonts' licences (`packaging/common.sh`).

- **Our product's name in our builds** (new file `crates/ui-egui/src/brand.rs`): `brand::name()` is
  `NEOFFICE_PRODUCT_NAME` when the build sets it (only the build step of `neoffice-web.yml`: `NeoPrés`), upstream's
  name otherwise, so upstream's builds and every test run are unchanged. DeckCraft has no string catalog, so the places
  that show the app's name call `brand::name()`, `brand::about()` or `brand::label()`: the title bar without a
  presentation (`ribbon.rs`), the About dialog and its title, the quit dialog's title, the Find dialog's "can't find"
  (`dialogs.rs`), the File tab's Home page (the start screen), its Account page and its Print page (`backstage.rs`),
  the Help menu and the command palette (`menus.rs`), the Presenter View's title (`show.rs`), two status messages
  (`lib.rs`: a file it can't open, a format it can't export) and the page that failed to start
  (`apps/deckcraft-web/src/web.rs`); `crates/engine/src/cmd/file.rs`, below the interface, applies the same rule
  (`product_name`) to its "not a presentation … can read" and "password-protected" errors, and `cmd/format.rs` to
  `font.add`'s. The page title and the
  loading text are ours in `apps/deckcraft-web/index.html`. The `DeckCraft` theme, storage folders, `deckcraft:*`
  messages, `ai.storyteller.*`, the file format's generator, the licence line and the native app keep upstream's name.

## Files that cannot carry a comment

- `Cargo.lock`: the Nora pane's dependencies, `craft-nora` and `craft-nora-ui` from `bvisible/craft-libs` (git, at the revision the
  workspace `Cargo.toml` names) and `sys-locale` 0.3.2 (the person's preferred
  languages). Nothing else moved. At an upstream merge, take upstream's lock and let cargo add these back (at the v0.9.0
  merge they were added by hand, the three packages and their three lines under `deckcraft-ui-egui`, and checked by the
  CI's `--locked` builds; at the v0.10.0 merge git's own merge of the lock kept them, and gave upstream's lock plus
  these 33 lines, nothing else).

## Upstream files left as they are

- `.github/workflows/nix.yml` and `.github/workflows/windows-arm64.yml` (upstream's, since v0.8): disabled on this fork
  through the API, like upstream's other workflows (they need upstream's signing, Nix's action, and a weekly Windows
  runner). `.github/dependabot.yml`: Dependabot's version updates do not run on a fork unless someone turns them on,
  which this fork does not do (they would open pull requests; this fork has one branch).
- `.github/workflows/sonarcloud.yml`, `sonar-project.properties` and `.coderabbit.yaml` (upstream's, since v0.10.0):
  SonarCloud's scan through `storytold/craft-common-actions` with upstream's project key and secrets, and CodeRabbit's
  review settings. The workflow is disabled on this fork through the API (it runs on upstream's `main` and on pull
  requests, neither of which this fork has); the two settings files do nothing without upstream's accounts.
