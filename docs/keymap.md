# Keymap

Generated from `crates/state/src/keymap.rs`; regenerate with `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.

## Custom bindings (not helix defaults)

| Keys | Context | Action |
| --- | --- | --- |
| `C-k C-s` | normal, insert, select, tree | Save all modified buffers. Shadows helix's insert-mode `C-k` (kill to line end). |
| `g left` / `g right` | normal, select | Line start / end (aliases of `gh` / `gl`). |
| `g up` / `g down` | normal, select | File start / last line (aliases of `gg` / `ge`). |
| `space e` | normal, select, tree | Toggle the file tree sidebar (helix opens an explorer picker). |
| `j k l h a r d esc` | tree | Move, open or expand, collapse, add, rename, delete (type `y`), back to the editor. |
| `g n` / `g p` | normal, select | Next / previous tab in the focused pane (helix: buffers). |

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
