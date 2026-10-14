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
| `space d` | normal, select | Toggle the tab between the file and its side-by-side diff (helix: diagnostics picker). The right side is the live buffer and takes every key. |
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
| `space v` | normal, select, rendered | Open a rendered view of the file in a split to the right that follows the text pane; again (from either side) closes it. Focus stays in the text. |
| `space V` | normal, select, rendered | Toggle the tab between the file's text and its rendered view in place, keeping the position. |
| `j` / `k`, down / up | rendered | Scroll a little. |
| `C-d` / `C-u` | rendered | Scroll half a page. |
| `C-f` / `C-b`, pagedown / pageup, `space space` | rendered | Scroll a page. |
| `gg` / `ge` / `G`, home / end | rendered | Top / bottom. |
| `]]` / `[[` | rendered | Next / previous top-level heading. |
| `esc` | rendered | Back to the text pane it follows; a standalone view toggles back to text. |
| `f` | rendered | Typst preview: fit pages to the pane width, or each page whole. |
| `+` / `-` / `=` | rendered | Typst preview: zoom pages in / out from the fit, or back to it. |
| `h` / `l`, left / right | rendered | Typst preview: pan sideways across pages wider than the pane. |
| `:export-pdf` | typst file | Compile the focused typst file to `<file>.pdf` beside it (also in the `space ?` palette). Markdown export is not supported yet. |
| mouse | rendered | Wheel scrolls. Click a block, or a spot on a typst page, to put the text cursor there (in the text pane it follows). Click a checkbox to toggle `[ ]` / `[x]` in the source (one undo step), a callout header to fold it, a link or a `![[note]]` embed to follow it (see links below). |
| `gf` | normal, select | Follow the link under the cursor: a wikilink, a markdown link or a URL in a markdown file; elsewhere the URL or file path under the cursor (or the selected text), relative to the file's folder, then the root, as in helix. |
| `space l` | normal, select, rendered | Backlinks: notes linking to the focused note, with the line of each link. Enter opens the note at that line; the preview shows it. Also `backlinks` in the `space ?` palette. |
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
| `:goto`, `:g` |
| `:reflow` |
| `:sort` |
| `:conflict-ours` |
| `:conflict-theirs` |
| `:conflict-both` |
| `:conflict-base` |
| `:export-pdf` |

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
| `esc` | `NormalMode` |
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
| `space d` | `ToggleDiff` |
| `] m` | `GotoConflict(Forward)` |
| `[ m` | `GotoConflict(Backward)` |
| `space m o` | `Resolve(Ours)` |
| `space m t` | `Resolve(Theirs)` |
| `space m b` | `Resolve(Both)` |
| `space m r` | `Resolve(Base)` |
| `space v` | `RenderedSplit` |
| `space V` | `ToggleRendered` |
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
| `esc` | `NormalMode` |
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
| `space d` | `ToggleDiff` |
| `] m` | `GotoConflict(Forward)` |
| `[ m` | `GotoConflict(Backward)` |
| `space m o` | `Resolve(Ours)` |
| `space m t` | `Resolve(Theirs)` |
| `space m b` | `Resolve(Both)` |
| `space m r` | `Resolve(Base)` |
| `space v` | `RenderedSplit` |
| `space V` | `ToggleRendered` |
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

## Rendered

| Keys | Command |
| --- | --- |
| `j` | `Rendered(Scroll(Forward, Lines))` |
| `down` | `Rendered(Scroll(Forward, Lines))` |
| `k` | `Rendered(Scroll(Backward, Lines))` |
| `up` | `Rendered(Scroll(Backward, Lines))` |
| `C-d` | `Rendered(Scroll(Forward, HalfPageCursor))` |
| `C-u` | `Rendered(Scroll(Backward, HalfPageCursor))` |
| `C-f` | `Rendered(Scroll(Forward, Page))` |
| `pagedown` | `Rendered(Scroll(Forward, Page))` |
| `space space` | `Rendered(Scroll(Forward, Page))` |
| `C-b` | `Rendered(Scroll(Backward, Page))` |
| `pageup` | `Rendered(Scroll(Backward, Page))` |
| `g g` | `Rendered(Top)` |
| `home` | `Rendered(Top)` |
| `g e` | `Rendered(Bottom)` |
| `G` | `Rendered(Bottom)` |
| `end` | `Rendered(Bottom)` |
| `] ]` | `Rendered(Heading(Forward))` |
| `[ [` | `Rendered(Heading(Backward))` |
| `esc` | `Rendered(Leave)` |
| `f` | `Rendered(Fit)` |
| `+` | `Rendered(Zoom(In))` |
| `-` | `Rendered(Zoom(Out))` |
| `=` | `Rendered(Zoom(Reset))` |
| `h` | `Rendered(Pan(Backward))` |
| `left` | `Rendered(Pan(Backward))` |
| `l` | `Rendered(Pan(Forward))` |
| `right` | `Rendered(Pan(Forward))` |
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `space v` | `RenderedSplit` |
| `space V` | `ToggleRendered` |
| `space l` | `Picker(Open(Backlinks))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
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
