# Keymap

Generated from `crates/state/src/keymap.rs`; regenerate with `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.

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

## Drawings

`.pdt` files are paged documents and `.bdt` files are boards; both open into panes like any file.
The pen draws and its eraser end erases on the tab's active layer; a pen-down focuses the pane.
Pages fit the pane's width at first; the page at the pane's centre is the page in focus.
Image files (png, jpeg, webp, gif, bmp, svg) open as one page fitted whole to the pane, read only.
PDFs open to be read, over their own pages: the sheet keys, `pn` / `pb` and `gg` / `ge` move through them. Dragging across text selects it, with a bar offering Highlight, Comment, Copy and Search; `v` highlights it, `i` highlights it with a comment, `y` copies it and `esc` lets it go. Tapping a highlight shows its comment; `i` edits the comment and `d` removes the highlight. `m` bookmarks the page in view, `space a` lists the PDF's highlights and bookmarks, and `space A` searches every annotation in the library. Annotations live in `.edt/library.db`, in the nearest folder above the project that has one, else the git repository's top, else the project. `/` finds text in the whole PDF, `n` / `N` step through the hits, and `space t` lists the table of contents. A tool key (`b`, `e`, `s`, `S`, `t…`) starts marking the PDF up with that tool, and `esc` goes back to reading. A PDF's pages can't be added or removed, and markup isn't written into the PDF yet.

| Keys | Context | Action |
| --- | --- | --- |
| `b` / `e` | drawing | The pen's tip draws / erases whole strokes. |
| `s` / `S` | drawing | The tip selects with a box / a lasso; a tap picks the mark under it. Drag the selection to move it, a corner to resize it keeping its shape, a side to stretch it (on text, to set where it wraps), the dot above to rotate. A tap on picked text types in it. |
| `tl` / `ta` / `tr` / `to` | drawing | The tip drags out a line / arrow / rectangle / ellipse. |
| `tt` | drawing | The tip places text: a tap starts it, a drag sets its wrap width, a press on text edits it. Typing uses helix keys in insert, normal and select mode; a press inside the text moves the cursor. |
| `esc` | text in normal mode | Keep the text and pick it, with the tip on box select; a press outside the text does the same and nothing more. Blank text is dropped. |
| `:image <path>`, `space i` | drawing, note | Put an image in: a drawing embeds it in the middle of the view, at its pixel size in points and no wider than most of the page, and it moves, turns and scales like any mark; a note links it as `![[name]]` on the cursor's line, or a new line below. `space i` picks from the project's images. |
| drop a file | drawing, note | An image dropped in the middle of a drawing lands where it was let go; on a note it is linked, copied beside the note first (lowercase, dashed name) when it comes from outside the project. Other files, and drops near an edge, open as before. |
| `space p`, `:paste-image` | drawing, note | Put the clipboard's image in. A note keeps it beside itself as `pasted-image-<date>.png` (UTC); `space p` in a note does the same when the clipboard holds an image and no text. |
| `dd`, `del` | drawing | Delete the selection. |
| `u` / `U` | drawing | Undo / redo the drawing. |
| `+` / `-` / `=` | pages, drawing, image | Zoom in / out around the centre / back to the fit. Ctrl+wheel zooms around the cursor. |
| `zf` | pages, drawing, image | Swap between fitting the pane's width and its height; an image, which opens fitted whole, goes to the one that fills more. |
| `h j k l`, arrows, wheel | pages, drawing, image | Pan; pages stay on screen. On typst pages `j`/`k` scroll and only `h`/`l` pan. |
| `pn` / `pb`, `gg` / `ge` | drawing | Next / previous page, first / last page. |
| `po` / `pO` | drawing | A blank page below / above the page in focus. |
| `pd` | drawing | Remove the page in focus. |
| `]l` / `[l` | drawing | Next / previous layer becomes active. |
| `space l h` / `space l n` | drawing | Hide or show the active layer / add a layer. |
| `:export-pdf` / `:export-svg` / `:export-png` | drawing | Write the drawing beside its file: a paged drawing's every page as PDF, or the page in focus as `<name>-<page>.svg` or `.png`; a board's picked marks, or all of it, with a margin. Hidden layers are left out; PDF text stays text, SVG and PNG text is drawn as outlines, and PNG is 2 pixels to the point. |

## Typed commands (`:`)

| Names |
| --- |
| `:write`, `:w` |
| `:write!`, `:w!` |
| `:write-all`, `:wa` |
| `:write-quit`, `:wq`, `:x` |
| `:write-quit-all`, `:wqa`, `:xa` |
| `:quit`, `:q` |
| `:quit-all`, `:qa` |
| `:quit!`, `:q!` |
| `:quit-all!`, `:qa!` |
| `:buffer-close`, `:bc`, `:bclose` |
| `:buffer-close!`, `:bc!`, `:bclose!` |
| `:reload`, `:rl` |
| `:reload-all`, `:rla` |
| `:buffer-next`, `:bn`, `:bnext` |
| `:buffer-previous`, `:bp`, `:bprev` |
| `:vsplit`, `:vs` |
| `:hsplit`, `:hs`, `:sp` |
| `:open`, `:o`, `:e`, `:edit` |
| `:image` |
| `:paste-image` |
| `:goto`, `:g` |
| `:reflow` |
| `:sort` |
| `:conflict-ours` |
| `:conflict-theirs` |
| `:conflict-both` |
| `:conflict-base` |
| `:export-pdf` |
| `:export-svg` |
| `:export-png` |

## Normal

| Keys | Command |
| --- | --- |
| `h` | `Move(CharLeft)` |
| `left` | `Move(CharLeft)` |
| `j` | `Move(VisualLineDown)` |
| `down` | `Move(VisualLineDown)` |
| `k` | `Move(VisualLineUp)` |
| `up` | `Move(VisualLineUp)` |
| `l` | `Move(CharRight)` |
| `right` | `Move(CharRight)` |
| `t` | `Await(Find(Find { direction: Forward, inclusion: Exclusive }))` |
| `f` | `Await(Find(Find { direction: Forward, inclusion: Inclusive }))` |
| `T` | `Await(Find(Find { direction: Backward, inclusion: Exclusive }))` |
| `F` | `Await(Find(Find { direction: Backward, inclusion: Inclusive }))` |
| `r` | `Await(Replace)` |
| `R` | `ReplaceWithYanked` |
| `A-.` | `RepeatMotion` |
| `~` | `ChangeCase(Switch)` |
| ``` | `ChangeCase(Lower)` |
| `A-`` | `ChangeCase(Upper)` |
| `home` | `Move(LineStart)` |
| `end` | `Move(LineEnd)` |
| `w` | `Move(NextWordStart)` |
| `b` | `Move(PrevWordStart)` |
| `e` | `Move(NextWordEnd)` |
| `W` | `Move(NextLongWordStart)` |
| `B` | `Move(PrevLongWordStart)` |
| `E` | `Move(NextLongWordEnd)` |
| `G` | `Move(Line)` |
| `g g` | `Move(FileStart)` |
| `g |` | `Move(Column)` |
| `g e` | `Move(LastLine)` |
| `g h` | `Move(LineStart)` |
| `g l` | `Move(LineEnd)` |
| `g s` | `Move(FirstNonWhitespace)` |
| `g k` | `Move(LineUp)` |
| `g j` | `Move(LineDown)` |
| `g .` | `Move(LastModification)` |
| `g f` | `GotoFile` |
| `g left` | `Move(LineStart)` |
| `g right` | `Move(LineEnd)` |
| `g up` | `Move(FileStart)` |
| `g down` | `Move(LastLine)` |
| `:` | `CommandMode` |
| `i` | `InsertMode` |
| `I` | `InsertAtLineStart` |
| `a` | `AppendMode` |
| `A` | `InsertAtLineEnd` |
| `o` | `OpenBelow` |
| `O` | `OpenAbove` |
| `d` | `Delete(Yank)` |
| `A-d` | `Delete(NoYank)` |
| `c` | `Change(Yank)` |
| `A-c` | `Change(NoYank)` |
| `C` | `CopySelectionOnLine(Forward)` |
| `A-C` | `CopySelectionOnLine(Backward)` |
| `s` | `RegexPrompt(Select)` |
| `A-s` | `SplitSelectionOnNewline` |
| `A--` | `MergeSelections` |
| `A-_` | `MergeConsecutiveSelections` |
| `S` | `RegexPrompt(Split)` |
| `;` | `CollapseSelection` |
| `A-;` | `FlipSelections` |
| `A-o` | `ExpandSelection` |
| `A-up` | `ExpandSelection` |
| `A-i` | `ShrinkSelection` |
| `A-down` | `ShrinkSelection` |
| `A-I` | `SelectAllChildren` |
| `A-S-down` | `SelectAllChildren` |
| `A-p` | `SelectSibling(Backward)` |
| `A-left` | `SelectSibling(Backward)` |
| `A-n` | `SelectSibling(Forward)` |
| `A-right` | `SelectSibling(Forward)` |
| `A-e` | `Move(ParentNodeEnd)` |
| `A-b` | `Move(ParentNodeStart)` |
| `A-a` | `SelectAllSiblings` |
| `%` | `SelectAll` |
| `x` | `ExtendLineBelow` |
| `X` | `ExtendToLineBounds` |
| `A-x` | `ShrinkToLineBounds` |
| `m m` | `Move(MatchingBracket)` |
| `m s` | `Await(SurroundAdd)` |
| `m r` | `Await(SurroundReplace)` |
| `m d` | `Await(SurroundDelete)` |
| `m a` | `Await(TextObject(Around))` |
| `m i` | `Await(TextObject(Inside))` |
| `[ f` | `GotoSyntaxObject(Function, Backward)` |
| `[ t` | `GotoSyntaxObject(Class, Backward)` |
| `[ a` | `GotoSyntaxObject(Parameter, Backward)` |
| `[ c` | `GotoSyntaxObject(Comment, Backward)` |
| `[ e` | `GotoSyntaxObject(Entry, Backward)` |
| `[ T` | `GotoSyntaxObject(Test, Backward)` |
| `[ x` | `GotoSyntaxObject(XmlElement, Backward)` |
| `[ p` | `Move(PrevParagraph)` |
| `[ space` | `AddNewline(Backward)` |
| `] f` | `GotoSyntaxObject(Function, Forward)` |
| `] t` | `GotoSyntaxObject(Class, Forward)` |
| `] a` | `GotoSyntaxObject(Parameter, Forward)` |
| `] c` | `GotoSyntaxObject(Comment, Forward)` |
| `] e` | `GotoSyntaxObject(Entry, Forward)` |
| `] T` | `GotoSyntaxObject(Test, Forward)` |
| `] x` | `GotoSyntaxObject(XmlElement, Forward)` |
| `] p` | `Move(NextParagraph)` |
| `] g` | `GotoHunk(Next)` |
| `[ g` | `GotoHunk(Previous)` |
| `] G` | `GotoHunk(Last)` |
| `[ G` | `GotoHunk(First)` |
| `space h s` | `Hunk(Stage)` |
| `space h u` | `Hunk(Unstage)` |
| `space h r` | `Hunk(Revert)` |
| `] space` | `AddNewline(Forward)` |
| `/` | `RegexPrompt(Search(Forward))` |
| `?` | `RegexPrompt(Search(Backward))` |
| `n` | `SearchNext(Forward)` |
| `N` | `SearchNext(Backward)` |
| `*` | `SearchSelection(Detect)` |
| `A-*` | `SearchSelection(Ignore)` |
| `u` | `Undo` |
| `U` | `Redo` |
| `A-u` | `Earlier` |
| `A-U` | `Later` |
| `y` | `Yank` |
| `p` | `Paste(After)` |
| `P` | `Paste(Before)` |
| `>` | `Indent` |
| `<` | `Unindent` |
| `J` | `Join(Plain)` |
| `A-J` | `Join(SelectSpace)` |
| `K` | `RegexPrompt(Keep)` |
| `A-K` | `RegexPrompt(Remove)` |
| `,` | `KeepPrimarySelection` |
| `A-,` | `RemovePrimarySelection` |
| `&` | `AlignSelections` |
| `_` | `TrimSelections` |
| `(` | `RotateSelections(Backward)` |
| `)` | `RotateSelections(Forward)` |
| `A-(` | `RotateSelectionContents(Backward)` |
| `A-)` | `RotateSelectionContents(Forward)` |
| `A-:` | `EnsureSelectionsForward` |
| `C-c` | `ToggleComments` |
| `"` | `Await(SelectRegister)` |
| `C-a` | `Increment` |
| `C-x` | `Decrement` |
| `.` | `RepeatInsert` |
| `space y` | `YankTo(Clipboard(System))` |
| `space Y` | `YankMainTo(Clipboard(System))` |
| `space p` | `PasteFrom(Clipboard(System), After)` |
| `space P` | `PasteFrom(Clipboard(System), Before)` |
| `space R` | `ReplaceFrom(Clipboard(System))` |
| `C-k C-s` | `WriteAll` |
| `esc` | `NormalMode` |
| `v` | `SelectMode` |
| `z z` | `AlignView(Center)` |
| `z c` | `AlignView(Center)` |
| `z t` | `AlignView(Top)` |
| `z b` | `AlignView(Bottom)` |
| `z k` | `Scroll(Backward, Lines)` |
| `z up` | `Scroll(Backward, Lines)` |
| `z j` | `Scroll(Forward, Lines)` |
| `z down` | `Scroll(Forward, Lines)` |
| `z C-b` | `Scroll(Backward, Page)` |
| `z pageup` | `Scroll(Backward, Page)` |
| `z C-f` | `Scroll(Forward, Page)` |
| `z pagedown` | `Scroll(Forward, Page)` |
| `z C-u` | `Scroll(Backward, HalfPageCursor)` |
| `z backspace` | `Scroll(Backward, HalfPageCursor)` |
| `z C-d` | `Scroll(Forward, HalfPageCursor)` |
| `z space` | `Scroll(Forward, HalfPageCursor)` |
| `z /` | `RegexPrompt(Search(Forward))` |
| `z ?` | `RegexPrompt(Search(Backward))` |
| `z n` | `SearchNext(Forward)` |
| `z N` | `SearchNext(Backward)` |
| `Z` | `StickyView` |
| `C-b` | `Scroll(Backward, Page)` |
| `pageup` | `Scroll(Backward, Page)` |
| `C-f` | `Scroll(Forward, Page)` |
| `pagedown` | `Scroll(Forward, Page)` |
| `C-u` | `Scroll(Backward, HalfPageCursor)` |
| `C-d` | `Scroll(Forward, HalfPageCursor)` |
| `g t` | `Move(Window(Top))` |
| `g c` | `Move(Window(Center))` |
| `g b` | `Move(Window(Bottom))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `space d` | `ToggleMode(Diff)` |
| `space v` | `ToggleMode(Split)` |
| `space V` | `ToggleMode(Render)` |
| `space tab` | `CycleMode(Forward)` |
| `space S-tab` | `CycleMode(Backward)` |
| `C-w <` | `ResizeSplit(Narrow)` |
| `C-w >` | `ResizeSplit(Widen)` |
| `C-w =` | `ResizeSplit(Even)` |
| `] m` | `GotoConflict(Forward)` |
| `[ m` | `GotoConflict(Backward)` |
| `space m o` | `Resolve(Ours)` |
| `space m t` | `Resolve(Theirs)` |
| `space m b` | `Resolve(Both)` |
| `space m r` | `Resolve(Base)` |
| `space l` | `Picker(Open(Backlinks))` |

## Insert

| Keys | Command |
| --- | --- |
| `esc` | `NormalMode` |
| `C-s` | `CommitUndoCheckpoint` |
| `C-r` | `Await(InsertRegister)` |
| `C-w` | `DeleteWordBackward` |
| `A-backspace` | `DeleteWordBackward` |
| `A-d` | `DeleteWordForward` |
| `A-del` | `DeleteWordForward` |
| `C-u` | `KillToLineStart` |
| `C-h` | `DeleteCharBackward` |
| `backspace` | `DeleteCharBackward` |
| `S-backspace` | `DeleteCharBackward` |
| `C-d` | `DeleteCharForward` |
| `del` | `DeleteCharForward` |
| `C-j` | `InsertNewline` |
| `ret` | `InsertNewline` |
| `tab` | `SmartTab` |
| `S-tab` | `InsertTab` |
| `up` | `Move(VisualLineUp)` |
| `down` | `Move(VisualLineDown)` |
| `left` | `Move(CharLeft)` |
| `right` | `Move(CharRight)` |
| `home` | `Move(LineStart)` |
| `end` | `Move(LineEndNewline)` |
| `pageup` | `Scroll(Backward, Page)` |
| `pagedown` | `Scroll(Forward, Page)` |
| `C-k C-s` | `WriteAll` |

## Select

| Keys | Command |
| --- | --- |
| `h` | `Move(CharLeft)` |
| `left` | `Move(CharLeft)` |
| `j` | `Move(VisualLineDown)` |
| `down` | `Move(VisualLineDown)` |
| `k` | `Move(VisualLineUp)` |
| `up` | `Move(VisualLineUp)` |
| `l` | `Move(CharRight)` |
| `right` | `Move(CharRight)` |
| `t` | `Await(Find(Find { direction: Forward, inclusion: Exclusive }))` |
| `f` | `Await(Find(Find { direction: Forward, inclusion: Inclusive }))` |
| `T` | `Await(Find(Find { direction: Backward, inclusion: Exclusive }))` |
| `F` | `Await(Find(Find { direction: Backward, inclusion: Inclusive }))` |
| `r` | `Await(Replace)` |
| `R` | `ReplaceWithYanked` |
| `A-.` | `RepeatMotion` |
| `~` | `ChangeCase(Switch)` |
| ``` | `ChangeCase(Lower)` |
| `A-`` | `ChangeCase(Upper)` |
| `home` | `Move(LineStart)` |
| `end` | `Move(LineEnd)` |
| `w` | `Move(NextWordStart)` |
| `b` | `Move(PrevWordStart)` |
| `e` | `Move(NextWordEnd)` |
| `W` | `Move(NextLongWordStart)` |
| `B` | `Move(PrevLongWordStart)` |
| `E` | `Move(NextLongWordEnd)` |
| `G` | `Move(Line)` |
| `g g` | `Move(FileStart)` |
| `g |` | `Move(Column)` |
| `g e` | `Move(LastLine)` |
| `g h` | `Move(LineStart)` |
| `g l` | `Move(LineEnd)` |
| `g s` | `Move(FirstNonWhitespace)` |
| `g k` | `Move(LineUp)` |
| `g j` | `Move(LineDown)` |
| `g .` | `Move(LastModification)` |
| `g f` | `GotoFile` |
| `g left` | `Move(LineStart)` |
| `g right` | `Move(LineEnd)` |
| `g up` | `Move(FileStart)` |
| `g down` | `Move(LastLine)` |
| `:` | `CommandMode` |
| `i` | `InsertMode` |
| `I` | `InsertAtLineStart` |
| `a` | `AppendMode` |
| `A` | `InsertAtLineEnd` |
| `o` | `OpenBelow` |
| `O` | `OpenAbove` |
| `d` | `Delete(Yank)` |
| `A-d` | `Delete(NoYank)` |
| `c` | `Change(Yank)` |
| `A-c` | `Change(NoYank)` |
| `C` | `CopySelectionOnLine(Forward)` |
| `A-C` | `CopySelectionOnLine(Backward)` |
| `s` | `RegexPrompt(Select)` |
| `A-s` | `SplitSelectionOnNewline` |
| `A--` | `MergeSelections` |
| `A-_` | `MergeConsecutiveSelections` |
| `S` | `RegexPrompt(Split)` |
| `;` | `CollapseSelection` |
| `A-;` | `FlipSelections` |
| `A-o` | `ExpandSelection` |
| `A-up` | `ExpandSelection` |
| `A-i` | `ShrinkSelection` |
| `A-down` | `ShrinkSelection` |
| `A-I` | `SelectAllChildren` |
| `A-S-down` | `SelectAllChildren` |
| `A-p` | `SelectSibling(Backward)` |
| `A-left` | `SelectSibling(Backward)` |
| `A-n` | `SelectSibling(Forward)` |
| `A-right` | `SelectSibling(Forward)` |
| `A-e` | `Move(ParentNodeEnd)` |
| `A-b` | `Move(ParentNodeStart)` |
| `A-a` | `SelectAllSiblings` |
| `%` | `SelectAll` |
| `x` | `ExtendLineBelow` |
| `X` | `ExtendToLineBounds` |
| `A-x` | `ShrinkToLineBounds` |
| `m m` | `Move(MatchingBracket)` |
| `m s` | `Await(SurroundAdd)` |
| `m r` | `Await(SurroundReplace)` |
| `m d` | `Await(SurroundDelete)` |
| `m a` | `Await(TextObject(Around))` |
| `m i` | `Await(TextObject(Inside))` |
| `[ f` | `GotoSyntaxObject(Function, Backward)` |
| `[ t` | `GotoSyntaxObject(Class, Backward)` |
| `[ a` | `GotoSyntaxObject(Parameter, Backward)` |
| `[ c` | `GotoSyntaxObject(Comment, Backward)` |
| `[ e` | `GotoSyntaxObject(Entry, Backward)` |
| `[ T` | `GotoSyntaxObject(Test, Backward)` |
| `[ x` | `GotoSyntaxObject(XmlElement, Backward)` |
| `[ p` | `Move(PrevParagraph)` |
| `[ space` | `AddNewline(Backward)` |
| `] f` | `GotoSyntaxObject(Function, Forward)` |
| `] t` | `GotoSyntaxObject(Class, Forward)` |
| `] a` | `GotoSyntaxObject(Parameter, Forward)` |
| `] c` | `GotoSyntaxObject(Comment, Forward)` |
| `] e` | `GotoSyntaxObject(Entry, Forward)` |
| `] T` | `GotoSyntaxObject(Test, Forward)` |
| `] x` | `GotoSyntaxObject(XmlElement, Forward)` |
| `] p` | `Move(NextParagraph)` |
| `] g` | `GotoHunk(Next)` |
| `[ g` | `GotoHunk(Previous)` |
| `] G` | `GotoHunk(Last)` |
| `[ G` | `GotoHunk(First)` |
| `space h s` | `Hunk(Stage)` |
| `space h u` | `Hunk(Unstage)` |
| `space h r` | `Hunk(Revert)` |
| `] space` | `AddNewline(Forward)` |
| `/` | `RegexPrompt(Search(Forward))` |
| `?` | `RegexPrompt(Search(Backward))` |
| `n` | `SearchNext(Forward)` |
| `N` | `SearchNext(Backward)` |
| `*` | `SearchSelection(Detect)` |
| `A-*` | `SearchSelection(Ignore)` |
| `u` | `Undo` |
| `U` | `Redo` |
| `A-u` | `Earlier` |
| `A-U` | `Later` |
| `y` | `Yank` |
| `p` | `Paste(After)` |
| `P` | `Paste(Before)` |
| `>` | `Indent` |
| `<` | `Unindent` |
| `J` | `Join(Plain)` |
| `A-J` | `Join(SelectSpace)` |
| `K` | `RegexPrompt(Keep)` |
| `A-K` | `RegexPrompt(Remove)` |
| `,` | `KeepPrimarySelection` |
| `A-,` | `RemovePrimarySelection` |
| `&` | `AlignSelections` |
| `_` | `TrimSelections` |
| `(` | `RotateSelections(Backward)` |
| `)` | `RotateSelections(Forward)` |
| `A-(` | `RotateSelectionContents(Backward)` |
| `A-)` | `RotateSelectionContents(Forward)` |
| `A-:` | `EnsureSelectionsForward` |
| `C-c` | `ToggleComments` |
| `"` | `Await(SelectRegister)` |
| `C-a` | `Increment` |
| `C-x` | `Decrement` |
| `.` | `RepeatInsert` |
| `space y` | `YankTo(Clipboard(System))` |
| `space Y` | `YankMainTo(Clipboard(System))` |
| `space p` | `PasteFrom(Clipboard(System), After)` |
| `space P` | `PasteFrom(Clipboard(System), Before)` |
| `space R` | `ReplaceFrom(Clipboard(System))` |
| `C-k C-s` | `WriteAll` |
| `esc` | `NormalMode` |
| `v` | `NormalMode` |
| `z z` | `AlignView(Center)` |
| `z c` | `AlignView(Center)` |
| `z t` | `AlignView(Top)` |
| `z b` | `AlignView(Bottom)` |
| `z k` | `Scroll(Backward, Lines)` |
| `z up` | `Scroll(Backward, Lines)` |
| `z j` | `Scroll(Forward, Lines)` |
| `z down` | `Scroll(Forward, Lines)` |
| `z C-b` | `Scroll(Backward, Page)` |
| `z pageup` | `Scroll(Backward, Page)` |
| `z C-f` | `Scroll(Forward, Page)` |
| `z pagedown` | `Scroll(Forward, Page)` |
| `z C-u` | `Scroll(Backward, HalfPageCursor)` |
| `z backspace` | `Scroll(Backward, HalfPageCursor)` |
| `z C-d` | `Scroll(Forward, HalfPageCursor)` |
| `z space` | `Scroll(Forward, HalfPageCursor)` |
| `z /` | `RegexPrompt(Search(Forward))` |
| `z ?` | `RegexPrompt(Search(Backward))` |
| `z n` | `SearchNext(Forward)` |
| `z N` | `SearchNext(Backward)` |
| `Z` | `StickyView` |
| `C-b` | `Scroll(Backward, Page)` |
| `pageup` | `Scroll(Backward, Page)` |
| `C-f` | `Scroll(Forward, Page)` |
| `pagedown` | `Scroll(Forward, Page)` |
| `C-u` | `Scroll(Backward, HalfPageCursor)` |
| `C-d` | `Scroll(Forward, HalfPageCursor)` |
| `g t` | `Move(Window(Top))` |
| `g c` | `Move(Window(Center))` |
| `g b` | `Move(Window(Bottom))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `space d` | `ToggleMode(Diff)` |
| `space v` | `ToggleMode(Split)` |
| `space V` | `ToggleMode(Render)` |
| `space tab` | `CycleMode(Forward)` |
| `space S-tab` | `CycleMode(Backward)` |
| `C-w <` | `ResizeSplit(Narrow)` |
| `C-w >` | `ResizeSplit(Widen)` |
| `C-w =` | `ResizeSplit(Even)` |
| `] m` | `GotoConflict(Forward)` |
| `[ m` | `GotoConflict(Backward)` |
| `space m o` | `Resolve(Ours)` |
| `space m t` | `Resolve(Theirs)` |
| `space m b` | `Resolve(Both)` |
| `space m r` | `Resolve(Base)` |
| `space l` | `Picker(Open(Backlinks))` |

## Prompt

| Keys | Command |
| --- | --- |
| `esc` | `PromptCancel` |
| `C-c` | `PromptCancel` |
| `ret` | `PromptSubmit` |
| `A-b` | `Prompt(Move(WordBackward))` |
| `C-left` | `Prompt(Move(WordBackward))` |
| `A-f` | `Prompt(Move(WordForward))` |
| `C-right` | `Prompt(Move(WordForward))` |
| `C-b` | `Prompt(Move(CharBackward))` |
| `left` | `Prompt(Move(CharBackward))` |
| `C-f` | `Prompt(Move(CharForward))` |
| `right` | `Prompt(Move(CharForward))` |
| `C-e` | `Prompt(Move(End))` |
| `end` | `Prompt(Move(End))` |
| `C-a` | `Prompt(Move(Start))` |
| `home` | `Prompt(Move(Start))` |
| `C-w` | `Prompt(Delete(WordBackward))` |
| `A-backspace` | `Prompt(Delete(WordBackward))` |
| `C-backspace` | `Prompt(Delete(WordBackward))` |
| `A-d` | `Prompt(Delete(WordForward))` |
| `A-del` | `Prompt(Delete(WordForward))` |
| `C-del` | `Prompt(Delete(WordForward))` |
| `C-k` | `Prompt(Delete(End))` |
| `C-u` | `Prompt(Delete(Start))` |
| `C-h` | `Prompt(Delete(CharBackward))` |
| `backspace` | `Prompt(Delete(CharBackward))` |
| `S-backspace` | `Prompt(Delete(CharBackward))` |
| `C-d` | `Prompt(Delete(CharForward))` |
| `del` | `Prompt(Delete(CharForward))` |
| `C-p` | `Prompt(History(Backward))` |
| `up` | `Prompt(History(Backward))` |
| `C-n` | `Prompt(History(Forward))` |
| `down` | `Prompt(History(Forward))` |
| `tab` | `Prompt(Complete(Forward))` |
| `S-tab` | `Prompt(Complete(Backward))` |

## Tree

| Keys | Command |
| --- | --- |
| `j` | `Tree(Down)` |
| `down` | `Tree(Down)` |
| `k` | `Tree(Up)` |
| `up` | `Tree(Up)` |
| `l` | `Tree(Open)` |
| `right` | `Tree(Open)` |
| `ret` | `Tree(Open)` |
| `h` | `Tree(Close)` |
| `left` | `Tree(Close)` |
| `a` | `Tree(Create)` |
| `r` | `Tree(Rename)` |
| `d` | `Tree(Delete)` |
| `esc` | `Tree(FocusEditor)` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |

## TreeInput

| Keys | Command |
| --- | --- |
| `esc` | `Tree(EditCancel)` |
| `C-c` | `Tree(EditCancel)` |
| `ret` | `Tree(EditSubmit)` |
| `backspace` | `Tree(EditBackspace)` |

## Picker

| Keys | Command |
| --- | --- |
| `esc` | `Picker(Cancel)` |
| `C-c` | `Picker(Cancel)` |
| `ret` | `Picker(Submit)` |
| `backspace` | `Picker(Backspace)` |
| `C-n` | `Picker(Next)` |
| `down` | `Picker(Next)` |
| `tab` | `Picker(Next)` |
| `C-p` | `Picker(Previous)` |
| `up` | `Picker(Previous)` |
| `S-tab` | `Picker(Previous)` |

## Changes

| Keys | Command |
| --- | --- |
| `j` | `Changes(Down)` |
| `down` | `Changes(Down)` |
| `k` | `Changes(Up)` |
| `up` | `Changes(Up)` |
| `h` | `Changes(Collapse)` |
| `left` | `Changes(Collapse)` |
| `l` | `Changes(Open)` |
| `right` | `Changes(Open)` |
| `ret` | `Changes(Open)` |
| `s` | `Changes(Stage)` |
| `u` | `Changes(Unstage)` |
| `d` | `Changes(Discard)` |
| `esc` | `Changes(FocusEditor)` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |

## Confirm

| Keys | Command |
| --- | --- |
| `y` | `Confirm(Yes)` |
| `n` | `Confirm(No)` |
| `esc` | `Confirm(No)` |
| `C-c` | `Confirm(No)` |

## Document

| Keys | Command |
| --- | --- |
| `j` | `Read(Move(Scroll(Forward, Lines)))` |
| `down` | `Read(Move(Scroll(Forward, Lines)))` |
| `k` | `Read(Move(Scroll(Backward, Lines)))` |
| `up` | `Read(Move(Scroll(Backward, Lines)))` |
| `C-d` | `Read(Move(Scroll(Forward, HalfPageCursor)))` |
| `C-u` | `Read(Move(Scroll(Backward, HalfPageCursor)))` |
| `C-f` | `Read(Move(Scroll(Forward, Page)))` |
| `pagedown` | `Read(Move(Scroll(Forward, Page)))` |
| `space space` | `Read(Move(Scroll(Forward, Page)))` |
| `C-b` | `Read(Move(Scroll(Backward, Page)))` |
| `pageup` | `Read(Move(Scroll(Backward, Page)))` |
| `g g` | `Read(Move(Top))` |
| `home` | `Read(Move(Top))` |
| `g e` | `Read(Move(Bottom))` |
| `G` | `Read(Move(Bottom))` |
| `end` | `Read(Move(Bottom))` |
| `esc` | `Read(Leave)` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `] ]` | `Document(Heading(Forward))` |
| `[ [` | `Document(Heading(Backward))` |
| `space d` | `ToggleMode(Diff)` |
| `space v` | `ToggleMode(Split)` |
| `space V` | `ToggleMode(Render)` |
| `space tab` | `CycleMode(Forward)` |
| `space S-tab` | `CycleMode(Backward)` |
| `C-w <` | `ResizeSplit(Narrow)` |
| `C-w >` | `ResizeSplit(Widen)` |
| `C-w =` | `ResizeSplit(Even)` |
| `space l` | `Picker(Open(Backlinks))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |

## Pages

| Keys | Command |
| --- | --- |
| `j` | `Read(Move(Scroll(Forward, Lines)))` |
| `down` | `Read(Move(Scroll(Forward, Lines)))` |
| `k` | `Read(Move(Scroll(Backward, Lines)))` |
| `up` | `Read(Move(Scroll(Backward, Lines)))` |
| `C-d` | `Read(Move(Scroll(Forward, HalfPageCursor)))` |
| `C-u` | `Read(Move(Scroll(Backward, HalfPageCursor)))` |
| `C-f` | `Read(Move(Scroll(Forward, Page)))` |
| `pagedown` | `Read(Move(Scroll(Forward, Page)))` |
| `space space` | `Read(Move(Scroll(Forward, Page)))` |
| `C-b` | `Read(Move(Scroll(Backward, Page)))` |
| `pageup` | `Read(Move(Scroll(Backward, Page)))` |
| `g g` | `Read(Move(Top))` |
| `home` | `Read(Move(Top))` |
| `g e` | `Read(Move(Bottom))` |
| `G` | `Read(Move(Bottom))` |
| `end` | `Read(Move(Bottom))` |
| `esc` | `Read(Leave)` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z f` | `View(Fit)` |
| `h` | `View(Pan(Left))` |
| `left` | `View(Pan(Left))` |
| `l` | `View(Pan(Right))` |
| `right` | `View(Pan(Right))` |
| `space d` | `ToggleMode(Diff)` |
| `space v` | `ToggleMode(Split)` |
| `space V` | `ToggleMode(Render)` |
| `space tab` | `CycleMode(Forward)` |
| `space S-tab` | `CycleMode(Backward)` |
| `C-w <` | `ResizeSplit(Narrow)` |
| `C-w >` | `ResizeSplit(Widen)` |
| `C-w =` | `ResizeSplit(Even)` |
| `space l` | `Picker(Open(Backlinks))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |

## Drawing

| Keys | Command |
| --- | --- |
| `b` | `Drawing(Ink(Tool(Brush)))` |
| `e` | `Drawing(Ink(Tool(Eraser)))` |
| `s` | `Drawing(Ink(Tool(BoxSelect)))` |
| `S` | `Drawing(Ink(Tool(LassoSelect)))` |
| `t l` | `Drawing(Ink(Tool(Line)))` |
| `t a` | `Drawing(Ink(Tool(Arrow)))` |
| `t r` | `Drawing(Ink(Tool(Rectangle)))` |
| `t o` | `Drawing(Ink(Tool(Ellipse)))` |
| `t t` | `Drawing(Ink(Tool(Text)))` |
| `p n` | `Drawing(Page(Go(Next)))` |
| `p b` | `Drawing(Page(Go(Previous)))` |
| `g g` | `Drawing(Page(Go(First)))` |
| `g e` | `Drawing(Page(Go(Last)))` |
| `p o` | `Drawing(Page(Insert(Below)))` |
| `p O` | `Drawing(Page(Insert(Above)))` |
| `p d` | `Drawing(Page(Remove))` |
| `space p` | `PasteImage` |
| `d d` | `Drawing(Ink(DeletePicked))` |
| `del` | `Drawing(Ink(DeletePicked))` |
| `u` | `Undo` |
| `U` | `Redo` |
| `] l` | `Drawing(Ink(Layer(Forward)))` |
| `[ l` | `Drawing(Ink(Layer(Backward)))` |
| `space l h` | `Drawing(Ink(ToggleLayer))` |
| `space l n` | `Drawing(Ink(NewLayer))` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z f` | `View(Fit)` |
| `h` | `View(Pan(Left))` |
| `left` | `View(Pan(Left))` |
| `l` | `View(Pan(Right))` |
| `right` | `View(Pan(Right))` |
| `k` | `View(Pan(Up))` |
| `up` | `View(Pan(Up))` |
| `j` | `View(Pan(Down))` |
| `down` | `View(Pan(Down))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |

## ImageViewer

| Keys | Command |
| --- | --- |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z f` | `View(Fit)` |
| `h` | `View(Pan(Left))` |
| `left` | `View(Pan(Left))` |
| `l` | `View(Pan(Right))` |
| `right` | `View(Pan(Right))` |
| `k` | `View(Pan(Up))` |
| `up` | `View(Pan(Up))` |
| `j` | `View(Pan(Down))` |
| `down` | `View(Pan(Down))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |

## DrawingText

| Keys | Command |
| --- | --- |
| `esc` | `Drawing(Ink(FinishText))` |
| `h` | `Move(CharLeft)` |
| `left` | `Move(CharLeft)` |
| `j` | `Move(VisualLineDown)` |
| `down` | `Move(VisualLineDown)` |
| `k` | `Move(VisualLineUp)` |
| `up` | `Move(VisualLineUp)` |
| `l` | `Move(CharRight)` |
| `right` | `Move(CharRight)` |
| `t` | `Await(Find(Find { direction: Forward, inclusion: Exclusive }))` |
| `f` | `Await(Find(Find { direction: Forward, inclusion: Inclusive }))` |
| `T` | `Await(Find(Find { direction: Backward, inclusion: Exclusive }))` |
| `F` | `Await(Find(Find { direction: Backward, inclusion: Inclusive }))` |
| `r` | `Await(Replace)` |
| `R` | `ReplaceWithYanked` |
| `A-.` | `RepeatMotion` |
| `~` | `ChangeCase(Switch)` |
| ``` | `ChangeCase(Lower)` |
| `A-`` | `ChangeCase(Upper)` |
| `home` | `Move(LineStart)` |
| `end` | `Move(LineEnd)` |
| `w` | `Move(NextWordStart)` |
| `b` | `Move(PrevWordStart)` |
| `e` | `Move(NextWordEnd)` |
| `W` | `Move(NextLongWordStart)` |
| `B` | `Move(PrevLongWordStart)` |
| `E` | `Move(NextLongWordEnd)` |
| `G` | `Move(Line)` |
| `g g` | `Move(FileStart)` |
| `g |` | `Move(Column)` |
| `g e` | `Move(LastLine)` |
| `g h` | `Move(LineStart)` |
| `g l` | `Move(LineEnd)` |
| `g s` | `Move(FirstNonWhitespace)` |
| `g k` | `Move(LineUp)` |
| `g j` | `Move(LineDown)` |
| `g .` | `Move(LastModification)` |
| `g f` | `GotoFile` |
| `g left` | `Move(LineStart)` |
| `g right` | `Move(LineEnd)` |
| `g up` | `Move(FileStart)` |
| `g down` | `Move(LastLine)` |
| `:` | `CommandMode` |
| `i` | `InsertMode` |
| `I` | `InsertAtLineStart` |
| `a` | `AppendMode` |
| `A` | `InsertAtLineEnd` |
| `o` | `OpenBelow` |
| `O` | `OpenAbove` |
| `d` | `Delete(Yank)` |
| `A-d` | `Delete(NoYank)` |
| `c` | `Change(Yank)` |
| `A-c` | `Change(NoYank)` |
| `C` | `CopySelectionOnLine(Forward)` |
| `A-C` | `CopySelectionOnLine(Backward)` |
| `s` | `RegexPrompt(Select)` |
| `A-s` | `SplitSelectionOnNewline` |
| `A--` | `MergeSelections` |
| `A-_` | `MergeConsecutiveSelections` |
| `S` | `RegexPrompt(Split)` |
| `;` | `CollapseSelection` |
| `A-;` | `FlipSelections` |
| `A-o` | `ExpandSelection` |
| `A-up` | `ExpandSelection` |
| `A-i` | `ShrinkSelection` |
| `A-down` | `ShrinkSelection` |
| `A-I` | `SelectAllChildren` |
| `A-S-down` | `SelectAllChildren` |
| `A-p` | `SelectSibling(Backward)` |
| `A-left` | `SelectSibling(Backward)` |
| `A-n` | `SelectSibling(Forward)` |
| `A-right` | `SelectSibling(Forward)` |
| `A-e` | `Move(ParentNodeEnd)` |
| `A-b` | `Move(ParentNodeStart)` |
| `A-a` | `SelectAllSiblings` |
| `%` | `SelectAll` |
| `x` | `ExtendLineBelow` |
| `X` | `ExtendToLineBounds` |
| `A-x` | `ShrinkToLineBounds` |
| `m m` | `Move(MatchingBracket)` |
| `m s` | `Await(SurroundAdd)` |
| `m r` | `Await(SurroundReplace)` |
| `m d` | `Await(SurroundDelete)` |
| `m a` | `Await(TextObject(Around))` |
| `m i` | `Await(TextObject(Inside))` |
| `[ f` | `GotoSyntaxObject(Function, Backward)` |
| `[ t` | `GotoSyntaxObject(Class, Backward)` |
| `[ a` | `GotoSyntaxObject(Parameter, Backward)` |
| `[ c` | `GotoSyntaxObject(Comment, Backward)` |
| `[ e` | `GotoSyntaxObject(Entry, Backward)` |
| `[ T` | `GotoSyntaxObject(Test, Backward)` |
| `[ x` | `GotoSyntaxObject(XmlElement, Backward)` |
| `[ p` | `Move(PrevParagraph)` |
| `[ space` | `AddNewline(Backward)` |
| `] f` | `GotoSyntaxObject(Function, Forward)` |
| `] t` | `GotoSyntaxObject(Class, Forward)` |
| `] a` | `GotoSyntaxObject(Parameter, Forward)` |
| `] c` | `GotoSyntaxObject(Comment, Forward)` |
| `] e` | `GotoSyntaxObject(Entry, Forward)` |
| `] T` | `GotoSyntaxObject(Test, Forward)` |
| `] x` | `GotoSyntaxObject(XmlElement, Forward)` |
| `] p` | `Move(NextParagraph)` |
| `] g` | `GotoHunk(Next)` |
| `[ g` | `GotoHunk(Previous)` |
| `] G` | `GotoHunk(Last)` |
| `[ G` | `GotoHunk(First)` |
| `space h s` | `Hunk(Stage)` |
| `space h u` | `Hunk(Unstage)` |
| `space h r` | `Hunk(Revert)` |
| `] space` | `AddNewline(Forward)` |
| `/` | `RegexPrompt(Search(Forward))` |
| `?` | `RegexPrompt(Search(Backward))` |
| `n` | `SearchNext(Forward)` |
| `N` | `SearchNext(Backward)` |
| `*` | `SearchSelection(Detect)` |
| `A-*` | `SearchSelection(Ignore)` |
| `u` | `Undo` |
| `U` | `Redo` |
| `A-u` | `Earlier` |
| `A-U` | `Later` |
| `y` | `Yank` |
| `p` | `Paste(After)` |
| `P` | `Paste(Before)` |
| `>` | `Indent` |
| `<` | `Unindent` |
| `J` | `Join(Plain)` |
| `A-J` | `Join(SelectSpace)` |
| `K` | `RegexPrompt(Keep)` |
| `A-K` | `RegexPrompt(Remove)` |
| `,` | `KeepPrimarySelection` |
| `A-,` | `RemovePrimarySelection` |
| `&` | `AlignSelections` |
| `_` | `TrimSelections` |
| `(` | `RotateSelections(Backward)` |
| `)` | `RotateSelections(Forward)` |
| `A-(` | `RotateSelectionContents(Backward)` |
| `A-)` | `RotateSelectionContents(Forward)` |
| `A-:` | `EnsureSelectionsForward` |
| `C-c` | `ToggleComments` |
| `"` | `Await(SelectRegister)` |
| `C-a` | `Increment` |
| `C-x` | `Decrement` |
| `.` | `RepeatInsert` |
| `space y` | `YankTo(Clipboard(System))` |
| `space Y` | `YankMainTo(Clipboard(System))` |
| `space p` | `PasteFrom(Clipboard(System), After)` |
| `space P` | `PasteFrom(Clipboard(System), Before)` |
| `space R` | `ReplaceFrom(Clipboard(System))` |
| `C-k C-s` | `WriteAll` |
| `v` | `SelectMode` |
| `z z` | `AlignView(Center)` |
| `z c` | `AlignView(Center)` |
| `z t` | `AlignView(Top)` |
| `z b` | `AlignView(Bottom)` |
| `z k` | `Scroll(Backward, Lines)` |
| `z up` | `Scroll(Backward, Lines)` |
| `z j` | `Scroll(Forward, Lines)` |
| `z down` | `Scroll(Forward, Lines)` |
| `z C-b` | `Scroll(Backward, Page)` |
| `z pageup` | `Scroll(Backward, Page)` |
| `z C-f` | `Scroll(Forward, Page)` |
| `z pagedown` | `Scroll(Forward, Page)` |
| `z C-u` | `Scroll(Backward, HalfPageCursor)` |
| `z backspace` | `Scroll(Backward, HalfPageCursor)` |
| `z C-d` | `Scroll(Forward, HalfPageCursor)` |
| `z space` | `Scroll(Forward, HalfPageCursor)` |
| `z /` | `RegexPrompt(Search(Forward))` |
| `z ?` | `RegexPrompt(Search(Backward))` |
| `z n` | `SearchNext(Forward)` |
| `z N` | `SearchNext(Backward)` |
| `Z` | `StickyView` |
| `C-b` | `Scroll(Backward, Page)` |
| `pageup` | `Scroll(Backward, Page)` |
| `C-f` | `Scroll(Forward, Page)` |
| `pagedown` | `Scroll(Forward, Page)` |
| `C-u` | `Scroll(Backward, HalfPageCursor)` |
| `C-d` | `Scroll(Forward, HalfPageCursor)` |
| `g t` | `Move(Window(Top))` |
| `g c` | `Move(Window(Center))` |
| `g b` | `Move(Window(Bottom))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |

## PdfRead

| Keys | Command |
| --- | --- |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `y` | `Drawing(Pdf(Copy))` |
| `/` | `Drawing(Pdf(Find))` |
| `space t` | `Picker(Open(Outline))` |
| `space a` | `Picker(Open(Notebook))` |
| `space A` | `Picker(Open(Annotations))` |
| `v` | `Drawing(Pdf(Annotate(Highlight)))` |
| `i` | `Drawing(Pdf(Annotate(Comment)))` |
| `d` | `Drawing(Pdf(Annotate(Remove)))` |
| `m` | `Drawing(Pdf(Annotate(Bookmark)))` |
| `n` | `Drawing(Pdf(FindNext(Forward)))` |
| `N` | `Drawing(Pdf(FindNext(Backward)))` |
| `esc` | `Drawing(Pdf(Read))` |
| `b` | `Drawing(Ink(Tool(Brush)))` |
| `e` | `Drawing(Ink(Tool(Eraser)))` |
| `s` | `Drawing(Ink(Tool(BoxSelect)))` |
| `S` | `Drawing(Ink(Tool(LassoSelect)))` |
| `t l` | `Drawing(Ink(Tool(Line)))` |
| `t a` | `Drawing(Ink(Tool(Arrow)))` |
| `t r` | `Drawing(Ink(Tool(Rectangle)))` |
| `t o` | `Drawing(Ink(Tool(Ellipse)))` |
| `t t` | `Drawing(Ink(Tool(Text)))` |
| `p n` | `Drawing(Page(Go(Next)))` |
| `p b` | `Drawing(Page(Go(Previous)))` |
| `g g` | `Drawing(Page(Go(First)))` |
| `g e` | `Drawing(Page(Go(Last)))` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z f` | `View(Fit)` |
| `h` | `View(Pan(Left))` |
| `left` | `View(Pan(Left))` |
| `l` | `View(Pan(Right))` |
| `right` | `View(Pan(Right))` |
| `k` | `View(Pan(Up))` |
| `up` | `View(Pan(Up))` |
| `j` | `View(Pan(Down))` |
| `down` | `View(Pan(Down))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |

## PdfMarkup

| Keys | Command |
| --- | --- |
| `esc` | `Drawing(Pdf(Read))` |
| `b` | `Drawing(Ink(Tool(Brush)))` |
| `e` | `Drawing(Ink(Tool(Eraser)))` |
| `s` | `Drawing(Ink(Tool(BoxSelect)))` |
| `S` | `Drawing(Ink(Tool(LassoSelect)))` |
| `t l` | `Drawing(Ink(Tool(Line)))` |
| `t a` | `Drawing(Ink(Tool(Arrow)))` |
| `t r` | `Drawing(Ink(Tool(Rectangle)))` |
| `t o` | `Drawing(Ink(Tool(Ellipse)))` |
| `t t` | `Drawing(Ink(Tool(Text)))` |
| `p n` | `Drawing(Page(Go(Next)))` |
| `p b` | `Drawing(Page(Go(Previous)))` |
| `g g` | `Drawing(Page(Go(First)))` |
| `g e` | `Drawing(Page(Go(Last)))` |
| `space p` | `PasteImage` |
| `d d` | `Drawing(Ink(DeletePicked))` |
| `del` | `Drawing(Ink(DeletePicked))` |
| `u` | `Undo` |
| `U` | `Redo` |
| `] l` | `Drawing(Ink(Layer(Forward)))` |
| `[ l` | `Drawing(Ink(Layer(Backward)))` |
| `space l h` | `Drawing(Ink(ToggleLayer))` |
| `space l n` | `Drawing(Ink(NewLayer))` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z f` | `View(Fit)` |
| `h` | `View(Pan(Left))` |
| `left` | `View(Pan(Left))` |
| `l` | `View(Pan(Right))` |
| `right` | `View(Pan(Right))` |
| `k` | `View(Pan(Up))` |
| `up` | `View(Pan(Up))` |
| `j` | `View(Pan(Down))` |
| `down` | `View(Pan(Down))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `space w v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w h` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w left` | `Window(Focus(Left))` |
| `space w down` | `Window(Focus(Down))` |
| `space w up` | `Window(Focus(Up))` |
| `space w right` | `Window(Focus(Right))` |
| `space w q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
