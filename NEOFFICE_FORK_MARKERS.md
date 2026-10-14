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

Three workflow files and one feature, all marked in place. The workflow files are new files that carry
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

## Files that cannot carry a comment

None so far.
