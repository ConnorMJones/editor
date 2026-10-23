//! The keymap reference in `docs/keymap.md`: prose kept by hand, tables generated.

use super::{Context, tables};

/// Bindings this editor adds or changes relative to helix, kept by hand; the tables are generated.
const CUSTOM: &str = "\
## Custom bindings (not helix defaults)

Source view keys are listed in their own section below.

| Keys | Context | Action |
| --- | --- | --- |
| `C-k C-s` | normal, insert, select, tree | Save all modified buffers. Shadows helix's insert-mode `C-k` (kill to line end). |
| `g left` / `g right` | normal, select | Line start / end (aliases of `gh` / `gl`). |
| `g up` / `g down` | normal, select | File start / last line (aliases of `gg` / `ge`). |
| `space e` | normal, select, tree | Toggle the file tree sidebar (helix opens an explorer picker). |
| `j k l h a r d esc` | tree | Move, open or expand, collapse, add, rename, delete (type `y`), back to the editor. |
| mouse | tree | Click to open, double-click to keep the tab. Right-click a row (or the empty area for the root): New File, New Folder, Rename, Delete. Drag a file or folder onto a folder (or onto a file, for its folder; the empty area for the root) to move it; open buffers follow. |
| `g n` / `g p` | normal, select | Next / previous tab in the focused pane (helix: buffers). |
";

/// Drawings, kept by hand.
const DRAWINGS: &str = "\
## Drawings

`.pdt` files are paged documents and `.bdt` files are boards; both open into panes like any file.
The pen draws and its eraser end erases on the tab's active layer; a pen-down focuses the pane.
Pages fit the pane's width at first; the page at the pane's centre is the page in focus.

| Keys | Context | Action |
| --- | --- | --- |
| `b` / `e` | drawing | The pen's tip draws / erases whole strokes. |
| `s` / `S` | drawing | The tip selects with a box / a lasso; a tap picks the mark under it. Drag the selection to move it, a corner to scale, the dot above to rotate. |
| `tl` / `ta` / `tr` / `to` | drawing | The tip drags out a line / arrow / rectangle / ellipse. |
| `dd`, `del` | drawing | Delete the selection. |
| `u` / `U` | drawing | Undo / redo the drawing. |
| `+` / `-` / `=` | drawing | Zoom in / out around the centre / back to the fit. Ctrl+wheel zooms around the cursor. |
| `zf` | drawing | Fit pages to the pane's width, or to its height. |
| `h j k l`, arrows, wheel | drawing | Pan; pages stay on screen. |
| `]p` / `[p`, `gg` / `G` | drawing | Next / previous page, first / last page. |
| `o` / `O` | drawing | A blank page below / above the page in focus. |
| `dp` | drawing | Remove the page in focus. |
| `]l` / `[l` | drawing | Next / previous layer becomes active. |
| `space l h` / `space l n` | drawing | Hide or show the active layer / add a layer. |
";

/// The git source view, kept by hand.
const SOURCE_VIEW: &str = "\
## Source view (git)

Diffs and gutter marks compare against HEAD; staged changes are dimmed. Colours are set under
`[theme.diff]` in the config (`added`, `removed`, `modified`, `ours`, `theirs`, `base`).

| Keys | Context | Action |
| --- | --- | --- |
| `space g` | normal, select, tree, changes | Toggle the Changes sidebar: Conflicts / Staged Changes / Changes (helix: changed-file picker). |
| `j` / `k` | changes | Move between files and group headers. |
| `l` / enter | changes | On a file: open its diff as a preview tab (conflicted files open as text; focus stays in the sidebar). On a header: collapse or expand the group. |
| `h` / left | changes | Collapse the group (from a file, move to its header). |
| `s` / `u` | changes | Stage / unstage the file, or the whole group on a header. |
| `d` | changes | Discard the file's changes (asks `y` / `n`; untracked files are deleted). |
| `esc` | changes | Back to the editor. |
| mouse | changes | Click a file to open its diff, double-click to keep the tab, click a header to collapse it. Hover a row for `+` stage, `−` unstage and `↶` discard. |
| `y` / `n` / `esc` | confirm | Answer a pending discard (or note creation, below). |
| `space d` | normal, select, document, pages | Toggle the tab between its source and its side-by-side diff (helix: diagnostics picker). The right side is the live buffer and takes every key. |
| `space tab` / `space S-tab` | normal, select, document, pages | Next / previous mode of the tab: source, then for `.md` and `.typ` files the split and the rendering, then the diff. |
| `]g` / `[g` | normal, select | Next / previous change; past the file's last / first change, continue into the next / previous changed file (its diff when in a diff; conflicted files open as text). Helix stops at the file's ends. |
| `]G` / `[G` | normal, select | Last / first change of the file, as helix. |
| `space h s` | normal, select | Stage the changes touched by the selections. |
| `space h u` | normal, select | Unstage the staged changes under the selections. |
| `space h r` | normal, select | Revert the changes under the selections to HEAD, as one undoable edit. |
| `]m` / `[m` | normal, select | Next / previous merge conflict, selecting it whole; select mode extends; takes a count. |
| `space m o` / `space m t` | normal, select | Resolve the conflicts touched by the selections with ours (HEAD) / theirs (incoming), as one undoable edit. Also `:conflict-ours` / `:conflict-theirs`. |
| `space m b` | normal, select | Resolve with both sides, ours then theirs. Also `:conflict-both`. |
| `space m r` | normal, select | Resolve with the base section of a diff3 conflict. Also `:conflict-base`. |
";

/// Rendered markdown and typst views, kept by hand.
const MARKDOWN_VIEW: &str = "\
## Markdown and typst

`.md` and `.typ` files have a rendered view: an Obsidian-like reading view of headings, lists,
task checkboxes, callouts (`> [!note]`, folded with `[!tip]-`, foldable with `[!tip]+`),
highlighted code, tables, images, wikilinks, math and mermaid. LaTeX math (`$inline$`,
`$$display$$`, ```` ```math ````) and ```` ```mermaid ```` diagrams are typeset in the
background in the theme's colours; until then, or when they fail, a muted box shows the source
(and the error). Inline math sits on the text baseline; its paragraph wraps between words.
Typst files show their pages, recompiled on every edit (imports and images resolve from the
project root). Errors show in a strip above the pages with the first error's
`file:line:col` and a count, and in the status line when they appear; the last good pages stay,
dimmed. Clicking the strip puts the text cursor on the error.

| Keys | Context | Action |
| --- | --- | --- |
| `space v` | normal, select, document, pages | Toggle the tab between its source and a split: the source on the left takes the keys, the rendering on the right follows its cursor. Scrolling the rendering scrolls the source; clicking it moves the cursor. |
| `C-w <` / `C-w >` / `C-w =` | normal, select | Narrow, widen or even out the source side of a split; dragging the divider between the sides resizes too, and double-clicking it evens them out. |
| `space V` | normal, select, document, pages | Toggle the tab between its source and its rendering, keeping the position. |
| `j` / `k`, down / up | document, pages | Scroll a little. |
| `C-d` / `C-u` | document, pages | Scroll half a page. |
| `C-f` / `C-b`, pagedown / pageup, `space space` | document, pages | Scroll a page. |
| `gg` / `ge` / `G`, home / end | document, pages | Top / bottom. |
| `]]` / `[[` | document | Next / previous top-level heading. |
| `esc` | document, pages | Back to the source. |
| `f` | pages | Fit pages to the pane width, or each page whole. |
| `+` / `-` / `=` | pages | Zoom pages in / out from the fit, or back to it. |
| `h` / `l`, left / right | pages | Pan sideways across pages wider than the pane. |
| `:export-pdf` | typst file | Compile the focused typst file to `<file>.pdf` beside it (also in the `space ?` palette). Markdown export is not supported yet. |
| mouse | document, pages | Wheel scrolls. Click a block, or a spot on a typst page, to put the text cursor there. Click a checkbox to toggle `[ ]` / `[x]` in the source (one undo step), a callout header to fold it, a link or a `![[note]]` embed to follow it (see links below). |
| `gf` | normal, select | Follow the link under the cursor: a wikilink, a markdown link or a URL in a markdown file; elsewhere the URL or file path under the cursor (or the selected text), relative to the file's folder, then the root, as in helix. |
| `space l` | normal, select, document, pages | Backlinks: notes linking to the focused note, with the line of each link. Enter opens the note at that line; the preview shows it. Also `backlinks` in the `space ?` palette. |
| `y` / `n` / `esc` | confirm | Create the note an unresolved link names, or not. |

A following view scrolls to the block under the text cursor, or under the text's top line
when it scrolls; scrolling the rendered view scrolls the text (not its cursor). A typst
preview scrolls to where the text cursor landed on the pages, and its strip shows the
first error or warning; clicking the strip opens it.

Links resolve the way Obsidian does, with the project root as the vault. `[[name]]` names
`name.md`, or a file named exactly `name` (`[[photo.png]]`), anywhere in the vault, ignoring
case; `[[folder/name]]` matches a path ending in it. When several files match, an exact-case
match wins, then the shallowest path, then one in the note's folder. Markdown links
(`[text](path.md)`) resolve against the note's folder, then the root, then by name.
`#heading` puts the cursor (or the rendered view) on that heading; `#^block` just opens the
note; `|alias` changes only the text shown. Links are followed into a rendered view when
followed from one, else into the text. Unresolved links render dimmed and in italics;
following one asks `create <path>? y/n` and creates an empty note next to the linking note
(or at the vault path the link names). `![[image.png]]` embeds images from anywhere in the
vault; `![[note]]` shows a box naming the resolved note that opens it when clicked.
";

/// Markdown listing of every binding, generated from the tables; `docs/keymap.md` holds a copy.
pub fn reference() -> String {
    let mut out = String::from(
        "# Keymap\n\nGenerated from `crates/state/src/keymap.rs`; regenerate with \
         `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.\n\n",
    );
    out.push_str(CUSTOM);
    out.push_str("\n");
    out.push_str(SOURCE_VIEW);
    out.push_str("\n");
    out.push_str(MARKDOWN_VIEW);
    out.push_str("\n");
    out.push_str(DRAWINGS);
    out.push_str("\n## Typed commands (`:`)\n\n| Names |\n| --- |\n");
    for (names, _) in crate::input::prompt::COMMANDS {
        let names: Vec<String> = names.iter().map(|name| format!("`:{name}`")).collect();
        out.push_str(&format!("| {} |\n", names.join(", ")));
    }
    for context in Context::ALL {
        out.push_str(&format!(
            "\n## {context:?}\n\n| Keys | Command |\n| --- | --- |\n"
        ));
        for (keys, command) in tables(context).iter().copied().flatten() {
            let keys: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
            out.push_str(&format!("| `{}` | `{command:?}` |\n", keys.join(" ")));
        }
    }
    out
}
