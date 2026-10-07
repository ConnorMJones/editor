use std::path::PathBuf;

/// Everything the user can ask the editor to do, from keys or the `:` prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    NormalMode,
    InsertMode,
    AppendMode,
    SelectMode,

    MoveCharLeft,
    MoveCharRight,
    MoveLineUp,
    MoveLineDown,

    InsertChar(char),
    InsertNewline,
    DeleteCharBackward,

    Undo,
    Redo,

    CommandMode,
    PromptInsert(char),
    PromptBackspace,
    PromptSubmit,
    PromptCancel,

    Open(PathBuf),
    Write,
    WriteAll,
    WriteQuit,
    Quit,
    ForceQuit,
    CloseBuffer,
    ForceCloseBuffer,
}
