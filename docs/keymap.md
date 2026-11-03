# Keymap

Generated from `crates/state/src/keymap.rs`; regenerate with `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.

## Custom bindings

| Keys | Context | Action |
| --- | --- | --- |
| `C-k C-s` | normal, insert, select, tree | Save all |
| `g left` / `g right` | normal, select | Line start / end |
| `g up` / `g down` | normal, select | File start / last line |
| `g n` / `g p` | normal, select | Next / previous tab |
| `space e` | normal, select, tree | Toggle the file tree |
| `j` / `k` | tree | Move |
| `S-up` / `S-down` | tree | Extend the selection |
| `l` / `h` | tree | Open or expand / collapse |
| `a` / `r` / `d` | tree | Add / rename / delete |
| `esc` | tree | Back to the editor |

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
| `space d` | normal, select, document, pages | Toggle the diff |
| `space tab` / `space S-tab` | normal, select, document, pages | Next / previous mode |
| `]g` / `[g` | normal, select | Next / previous change, across files |
| `]G` / `[G` | normal, select | Last / first change |
| `space h s` / `space h u` / `space h r` | normal, select | Stage / unstage / revert the change |
| `]m` / `[m` | normal, select | Next / previous conflict |
| `space m o` / `space m t` | normal, select | Take ours / theirs |
| `space m b` / `space m r` | normal, select | Take both / base |

## Markdown and typst

| Keys | Context | Action |
| --- | --- | --- |
| `space v` | normal, select, document, pages | Toggle the split |
| `space V` | normal, select, document, pages | Toggle the rendering |
| `C-w <` / `C-w >` / `C-w =` | normal, select | Narrow / widen / even the split |
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
| `+` / `-` / `=` | pages, drawing, image, PDF, canvas | Zoom in / out / fit |
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
| `space v` | comment, card | Full / split |
| `d` | PDF, book | Delete the highlight |
| `y` | PDF, book | Copy |
| `esc` | PDF, book | Clear the selection |
| `m` | PDF, book | Toggle a bookmark |
| `/` | PDF, book | Find |
| `n` / `N` | PDF, book | Next / previous hit |
| `space a` | PDF, book | Notebook |
| `space A` | PDF, book | Search annotations |
| `space t` | PDF, book | Contents |
| `space tab`, `space d` | PDF | Read / markup |
| `:flatten` | PDF | Flatten the markup |
| `:export-notes` | PDF, book | Write annotations to a note beside it |
| `]]` / `[[` | book | Next / previous heading |
| `d`, `del` | canvas | Delete |
| `y` | canvas | Duplicate |
| `at`, double click | canvas | New text card |
| `i`, `ret`, double click | canvas | Write in the card |
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
| `:` | `CommandMode` |
| `C-k C-s` | `WriteAll` |
| `esc` | `Read(Leave)` |
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

## JsonCanvas

| Keys | Command |
| --- | --- |
| `d` | `JsonCanvas(Delete)` |
| `del` | `JsonCanvas(Delete)` |
| `y` | `JsonCanvas(Duplicate)` |
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
| `space tab` | `Drawing(Pdf(Markup))` |
| `space d` | `Drawing(Pdf(Markup))` |
| `C-k C-s` | `WriteAll` |
| `y` | `Reading(Copy)` |
| `/` | `Reading(Find)` |
| `space t` | `Picker(Open(Outline))` |
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
| `space tab` | `Drawing(Pdf(Read))` |
| `space d` | `Drawing(Pdf(Read))` |
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
| `y` | `Reading(Copy)` |
| `/` | `Reading(Find)` |
| `space t` | `Picker(Open(Outline))` |
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

## Draft

| Keys | Command |
| --- | --- |
| `C-v` | `Draft(Expand)` |
| `esc` | `Draft(Finish)` |
| `C-s` | `Write` |
| `space q` | `Draft(Drop)` |
| `space v` | `Draft(Swap)` |
| `C-w <` | `ResizeSplit(Narrow)` |
| `C-w >` | `ResizeSplit(Widen)` |
| `C-w =` | `ResizeSplit(Even)` |
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
