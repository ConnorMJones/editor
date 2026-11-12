# Keymap

Generated from `crates/state/src/input/keymap/tables.rs`; regenerate with `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.

## Custom bindings

| Keys | Context | Action |
| --- | --- | --- |
| `C-k C-s` | normal, insert, select, tree | Save all |
| `g left` / `g right` | normal, select | Line start / end |
| `g up` / `g down` | normal, select | File start / last line |
| `space e` | normal, select, tree | Toggle the file tree |
| `j` / `k` | tree | Move |
| `S-up` / `S-down` | tree | Extend the selection |
| `l` / `h` | tree | Open or expand / collapse |
| `a` / `r` / `d` | tree | Add / rename / delete |
| `esc` | tree | Back to the editor |

## Panes, tabs and views

| Keys | Action |
| --- | --- |
| `C-w v` / `C-w s` | Split right / below |
| `C-w w` | Next pane |
| `C-w h j k l`, arrows | Focus |
| `C-w H J K L` | Swap with the pane on that side |
| `C-w t` | Transpose the split |
| `C-w f` / `C-w F` | Open the file under the cursor below / right |
| `C-w q` / `C-w o` | Close / keep only this pane |
| `C-w <` / `C-w >` | Narrower / wider |
| `C-w -` / `C-w +` | Shorter / taller |
| `C-w =` | Even all panes |
| `C-w _` / `C-w \|` | Tallest / widest |
| `C-w z` | Maximise / restore |
| `C-w m` `h j k l` | Move the tab to that side |
| `space w` … | Same as `C-w` … |
| `gn` / `gp` | Next / previous tab |
| `ga` | Last used tab |
| `g<` / `g>` | Move the tab left / right |
| `:bc` / `:bco` / `:bca` | Close this / other / all tabs |
| `zr` | Rendered |
| `zv` | Source and rendered side by side |
| `zd` | Diff |
| `zs` | Source |
| `z tab` / `z S-tab` | Next / previous view |
| `z<` / `z>` / `z=` | Move the divider |

## Source view (git)

| Keys | Context | Action |
| --- | --- | --- |
| `space g` | normal, select, tree, changes | Toggle the Changes sidebar |
| `j` / `k` | changes | Move |
| `S-up` / `S-down` | changes | Extend the selection |
| `l` / enter | changes | Open the diff / fold the group |
| `h` / left | changes | Fold the group |
| `s` / `u` | changes | Stage / unstage |
| `d` | changes | Discard |
| `esc` | changes | Back to the editor |
| `y` / `n` / `esc` | confirm | Yes / no |
| `]g` / `[g` | normal, select | Next / previous change, across files |
| `]G` / `[G` | normal, select | Last / first change |
| `space h s` / `space h u` / `space h r` | normal, select | Stage / unstage / revert the change |
| `]m` / `[m` | normal, select | Next / previous conflict |
| `space m o` / `space m t` | normal, select | Take ours / theirs |
| `space m b` / `space m r` | normal, select | Take both / base |

## Markdown and typst

| Keys | Context | Action |
| --- | --- | --- |
| `j` / `k`, down / up | document, pages | Scroll |
| `C-d` / `C-u` | document, pages | Half page |
| `C-f` / `C-b`, pagedown / pageup, `space space` | document, pages | Page |
| `gg` / `ge` / `G`, home / end | document, pages | Top / bottom |
| `]]` / `[[` | document | Next / previous heading |
| `esc` | document, pages | Back to the source |
| `h` / `l`, left / right | pages | Pan |
| `gf` | normal, select | Follow the link |
| `space l` | normal, select, document, pages | Backlinks |
| `:export-pdf` | typst | Export to PDF |

## Drawings, images, PDFs, books and canvases

| Keys | Context | Action |
| --- | --- | --- |
| `b` / `e` | drawing | Brush / eraser |
| `s` / `S` | drawing | Box / lasso select |
| `tl` / `ta` / `tr` / `to` | drawing | Line / arrow / rectangle / ellipse |
| `tt` | drawing | Text |
| `esc` | drawing text | Finish the text |
| `dd`, `del` | drawing | Delete the selection |
| `u` / `U` | drawing | Undo / redo |
| `pn` / `pb` | drawing, PDF | Next / previous page |
| `gg` / `ge` | drawing, PDF, book | First / last page |
| `po` / `pO` | drawing | New page below / above |
| `pd` | drawing | Remove the page |
| `]l` / `[l` | drawing | Next / previous layer |
| `space l h` / `space l n` | drawing | Hide / add a layer |
| `space i`, `:image <path>` | drawing, note | Insert an image |
| `space p`, `:paste-image` | drawing, note | Paste an image |
| `+` / `-` / `=` | rendered note, book, pages, drawing, image, PDF, canvas | Zoom in / out / reset |
| `zf` | pages, drawing, image, PDF, canvas | Fit width / height |
| `h j k l`, arrows | pages, drawing, image, PDF, canvas | Pan |
| `:export-pdf` / `:export-svg` / `:export-png` | drawing | Export |
| `v` | PDF, book | Highlight |
| `i` | PDF, book | Highlight with a comment / edit the comment |
| `I` | PDF, book | Write the comment beside the page |
| `C-v` | comment prompt | Move to the full view |
| `esc` | comment | Save and close |
| `C-s`, `:w` | comment, card | Save |
| `space q` | comment, card | Close without saving |
| `zv` | comment, card | Full / split |
| `z<` / `z>` / `z=` | comment, card | Narrower / wider / even |
| `d` | PDF, book | Delete the highlight |
| `y` | PDF, book | Copy |
| `esc` | PDF, book | Clear the selection |
| `m` | PDF, book | Toggle a bookmark |
| `/` | PDF, book | Find |
| `n` / `N` | PDF, book | Next / previous hit |
| `space a` | PDF, book | Notebook |
| `space A` | PDF, book | Search annotations |
| `space o` | PDF, book | Contents |
| `z tab` | PDF | Read / markup |
| `:flatten` | PDF | Flatten the markup |
| `:export-notes` | PDF, book | Write annotations to a note beside it |
| `]]` / `[[` | book | Next / previous heading |
| `d`, `del` | canvas | Delete |
| `y` | canvas | Duplicate |
| `zp` | canvas | Zoom to the selection |
| `at`, double click | canvas | New text card |
| `i`, `ret`, double click | canvas | Write in the card or label |
| middle drag | canvas | Pan |
| `esc` | card | Keep the text and close |
| `C-v` | card | In the card / beside |
| `ag` | canvas | Group the selection |
| drag a side dot | canvas | Connect |
| `c1` … `c6` / `c0` | canvas | Colour / no colour |
| `u` / `U` | canvas | Undo / redo |
| `esc` | canvas | Clear the selection |
| shift click | canvas | Add to the selection |
| alt drag | canvas | Move without the grid |

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
| `:buffer-close-others`, `:bco`, `:bcloseother` |
| `:buffer-close-others!`, `:bco!`, `:bcloseother!` |
| `:buffer-close-all`, `:bca`, `:bcloseall` |
| `:buffer-close-all!`, `:bca!`, `:bcloseall!` |
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
| `:flatten` |
| `:export-notes` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |
| `z r` | `ToggleMode(Render)` |
| `z v` | `ToggleMode(Split)` |
| `z d` | `ToggleMode(Diff)` |
| `z s` | `ToggleMode(Source)` |
| `z tab` | `CycleMode(Forward)` |
| `z S-tab` | `CycleMode(Backward)` |
| `z <` | `ResizeSplit(Narrow)` |
| `z >` | `ResizeSplit(Widen)` |
| `z =` | `ResizeSplit(Even)` |
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
| `C-s` | `CommitUndoCheckpoint` |
| `esc` | `NormalMode` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |
| `z r` | `ToggleMode(Render)` |
| `z v` | `ToggleMode(Split)` |
| `z d` | `ToggleMode(Diff)` |
| `z s` | `ToggleMode(Source)` |
| `z tab` | `CycleMode(Forward)` |
| `z S-tab` | `CycleMode(Backward)` |
| `z <` | `ResizeSplit(Narrow)` |
| `z >` | `ResizeSplit(Widen)` |
| `z =` | `ResizeSplit(Even)` |
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
| `C-v` | `Draft(Expand)` |
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
| `S-down` | `Tree(Extend(Forward))` |
| `S-up` | `Tree(Extend(Backward))` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |

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
| `S-down` | `Changes(Extend(Forward))` |
| `S-up` | `Changes(Extend(Backward))` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |

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
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `esc` | `Read(Leave)` |
| `] ]` | `Document(Heading(Forward))` |
| `[ [` | `Document(Heading(Backward))` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z r` | `ToggleMode(Render)` |
| `z v` | `ToggleMode(Split)` |
| `z d` | `ToggleMode(Diff)` |
| `z s` | `ToggleMode(Source)` |
| `z tab` | `CycleMode(Forward)` |
| `z S-tab` | `CycleMode(Backward)` |
| `z <` | `ResizeSplit(Narrow)` |
| `z >` | `ResizeSplit(Widen)` |
| `z =` | `ResizeSplit(Even)` |
| `space l` | `Picker(Open(Backlinks))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

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
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `esc` | `Read(Leave)` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `z f` | `View(Fit)` |
| `h` | `View(Pan(Left))` |
| `left` | `View(Pan(Left))` |
| `l` | `View(Pan(Right))` |
| `right` | `View(Pan(Right))` |
| `z r` | `ToggleMode(Render)` |
| `z v` | `ToggleMode(Split)` |
| `z d` | `ToggleMode(Diff)` |
| `z s` | `ToggleMode(Source)` |
| `z tab` | `CycleMode(Forward)` |
| `z S-tab` | `CycleMode(Backward)` |
| `z <` | `ResizeSplit(Narrow)` |
| `z >` | `ResizeSplit(Widen)` |
| `z =` | `ResizeSplit(Even)` |
| `space l` | `Picker(Open(Backlinks))` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

## JsonCanvas

| Keys | Command |
| --- | --- |
| `d` | `JsonCanvas(Delete)` |
| `del` | `JsonCanvas(Delete)` |
| `y` | `JsonCanvas(Duplicate)` |
| `z p` | `JsonCanvas(Focus)` |
| `i` | `JsonCanvas(Write)` |
| `ret` | `JsonCanvas(Write)` |
| `a t` | `JsonCanvas(AddText)` |
| `a g` | `JsonCanvas(Group)` |
| `c 0` | `JsonCanvas(Recolor(None))` |
| `c 1` | `JsonCanvas(Recolor(Some(Red)))` |
| `c 2` | `JsonCanvas(Recolor(Some(Orange)))` |
| `c 3` | `JsonCanvas(Recolor(Some(Yellow)))` |
| `c 4` | `JsonCanvas(Recolor(Some(Green)))` |
| `c 5` | `JsonCanvas(Recolor(Some(Cyan)))` |
| `c 6` | `JsonCanvas(Recolor(Some(Purple)))` |
| `esc` | `JsonCanvas(Unpick)` |
| `u` | `Undo` |
| `U` | `Redo` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

## PdfRead

| Keys | Command |
| --- | --- |
| `:` | `CommandMode` |
| `z tab` | `Drawing(Pdf(Markup))` |
| `C-k C-s` | `WriteAll` |
| `y` | `Reading(Copy)` |
| `/` | `Reading(Find)` |
| `space o` | `Picker(Open(Outline))` |
| `space a` | `Picker(Open(Notebook))` |
| `space A` | `Picker(Open(Annotations))` |
| `v` | `Reading(Annotate(Highlight))` |
| `i` | `Reading(Annotate(Comment))` |
| `I` | `Reading(Annotate(Write))` |
| `d` | `Reading(Annotate(Remove))` |
| `m` | `Reading(Annotate(Bookmark))` |
| `n` | `Reading(FindNext(Forward))` |
| `N` | `Reading(FindNext(Backward))` |
| `esc` | `Reading(Clear)` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

## PdfMarkup

| Keys | Command |
| --- | --- |
| `esc` | `Drawing(Pdf(Read))` |
| `z tab` | `Drawing(Pdf(Read))` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

## Book

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
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `] ]` | `Document(Heading(Forward))` |
| `[ [` | `Document(Heading(Backward))` |
| `+` | `View(Zoom(In))` |
| `-` | `View(Zoom(Out))` |
| `=` | `View(Zoom(Reset))` |
| `y` | `Reading(Copy)` |
| `/` | `Reading(Find)` |
| `space o` | `Picker(Open(Outline))` |
| `space a` | `Picker(Open(Notebook))` |
| `space A` | `Picker(Open(Annotations))` |
| `v` | `Reading(Annotate(Highlight))` |
| `i` | `Reading(Annotate(Comment))` |
| `I` | `Reading(Annotate(Write))` |
| `d` | `Reading(Annotate(Remove))` |
| `m` | `Reading(Annotate(Bookmark))` |
| `n` | `Reading(FindNext(Forward))` |
| `N` | `Reading(FindNext(Backward))` |
| `esc` | `Reading(Clear)` |
| `space e` | `Tree(Toggle)` |
| `space f` | `Picker(Open(Files))` |
| `space i` | `Picker(Open(Images))` |
| `space b` | `Picker(Open(Buffers))` |
| `space /` | `Picker(Open(Grep))` |
| `space ?` | `Picker(Open(Commands))` |
| `space g` | `Changes(Toggle)` |
| `C-w v` | `Window(Split(Row))` |
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

## Draft

| Keys | Command |
| --- | --- |
| `C-v` | `Draft(Expand)` |
| `esc` | `Draft(Finish)` |
| `C-s` | `Write` |
| `space q` | `Draft(Drop)` |
| `z v` | `Draft(Swap)` |
| `z <` | `ResizeSplit(Narrow)` |
| `z >` | `ResizeSplit(Widen)` |
| `z =` | `ResizeSplit(Even)` |
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
| `C-w C-v` | `Window(Split(Row))` |
| `C-w s` | `Window(Split(Column))` |
| `C-w C-s` | `Window(Split(Column))` |
| `C-w w` | `Window(FocusNext)` |
| `C-w C-w` | `Window(FocusNext)` |
| `C-w t` | `Window(Transpose)` |
| `C-w C-t` | `Window(Transpose)` |
| `C-w f` | `Window(GotoFile(Column))` |
| `C-w F` | `Window(GotoFile(Row))` |
| `C-w q` | `Window(Close)` |
| `C-w C-q` | `Window(Close)` |
| `C-w o` | `Window(Only)` |
| `C-w C-o` | `Window(Only)` |
| `C-w h` | `Window(Focus(Left))` |
| `C-w C-h` | `Window(Focus(Left))` |
| `C-w left` | `Window(Focus(Left))` |
| `C-w j` | `Window(Focus(Down))` |
| `C-w C-j` | `Window(Focus(Down))` |
| `C-w down` | `Window(Focus(Down))` |
| `C-w k` | `Window(Focus(Up))` |
| `C-w C-k` | `Window(Focus(Up))` |
| `C-w up` | `Window(Focus(Up))` |
| `C-w l` | `Window(Focus(Right))` |
| `C-w C-l` | `Window(Focus(Right))` |
| `C-w right` | `Window(Focus(Right))` |
| `C-w H` | `Window(Swap(Left))` |
| `C-w J` | `Window(Swap(Down))` |
| `C-w K` | `Window(Swap(Up))` |
| `C-w L` | `Window(Swap(Right))` |
| `C-w <` | `Window(Resize(Narrower))` |
| `C-w >` | `Window(Resize(Wider))` |
| `C-w -` | `Window(Resize(Shorter))` |
| `C-w +` | `Window(Resize(Taller))` |
| `C-w =` | `Window(Resize(Even))` |
| `C-w _` | `Window(Resize(TallestHere))` |
| `C-w |` | `Window(Resize(WidestHere))` |
| `C-w z` | `Window(Maximise)` |
| `C-w m h` | `Window(MoveTab(Left))` |
| `C-w m j` | `Window(MoveTab(Down))` |
| `C-w m k` | `Window(MoveTab(Up))` |
| `C-w m l` | `Window(MoveTab(Right))` |
| `space w v` | `Window(Split(Row))` |
| `space w C-v` | `Window(Split(Row))` |
| `space w s` | `Window(Split(Column))` |
| `space w C-s` | `Window(Split(Column))` |
| `space w w` | `Window(FocusNext)` |
| `space w C-w` | `Window(FocusNext)` |
| `space w t` | `Window(Transpose)` |
| `space w C-t` | `Window(Transpose)` |
| `space w f` | `Window(GotoFile(Column))` |
| `space w F` | `Window(GotoFile(Row))` |
| `space w q` | `Window(Close)` |
| `space w C-q` | `Window(Close)` |
| `space w o` | `Window(Only)` |
| `space w C-o` | `Window(Only)` |
| `space w h` | `Window(Focus(Left))` |
| `space w C-h` | `Window(Focus(Left))` |
| `space w left` | `Window(Focus(Left))` |
| `space w j` | `Window(Focus(Down))` |
| `space w C-j` | `Window(Focus(Down))` |
| `space w down` | `Window(Focus(Down))` |
| `space w k` | `Window(Focus(Up))` |
| `space w C-k` | `Window(Focus(Up))` |
| `space w up` | `Window(Focus(Up))` |
| `space w l` | `Window(Focus(Right))` |
| `space w C-l` | `Window(Focus(Right))` |
| `space w right` | `Window(Focus(Right))` |
| `space w H` | `Window(Swap(Left))` |
| `space w J` | `Window(Swap(Down))` |
| `space w K` | `Window(Swap(Up))` |
| `space w L` | `Window(Swap(Right))` |
| `space w <` | `Window(Resize(Narrower))` |
| `space w >` | `Window(Resize(Wider))` |
| `space w -` | `Window(Resize(Shorter))` |
| `space w +` | `Window(Resize(Taller))` |
| `space w =` | `Window(Resize(Even))` |
| `space w _` | `Window(Resize(TallestHere))` |
| `space w |` | `Window(Resize(WidestHere))` |
| `space w z` | `Window(Maximise)` |
| `space w m h` | `Window(MoveTab(Left))` |
| `space w m j` | `Window(MoveTab(Down))` |
| `space w m k` | `Window(MoveTab(Up))` |
| `space w m l` | `Window(MoveTab(Right))` |
| `g n` | `CycleBuffer(Next)` |
| `g p` | `CycleBuffer(Previous)` |
| `g a` | `Tab(LastUsed)` |
| `g <` | `Tab(Shift(Backward))` |
| `g >` | `Tab(Shift(Forward))` |

## DraftInsert

| Keys | Command |
| --- | --- |
| `C-s` | `Write` |
| `esc` | `NormalMode` |
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
