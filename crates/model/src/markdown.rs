#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CalloutKind {
    Note,
    Abstract,
    Info,
    Todo,
    Tip,
    Success,
    Question,
    Warning,
    Failure,
    Danger,
    Bug,
    Example,
    Quote,
}

impl CalloutKind {
    pub const COUNT: usize = 13;
    pub const ALL: [CalloutKind; CalloutKind::COUNT] = [
        CalloutKind::Note,
        CalloutKind::Abstract,
        CalloutKind::Info,
        CalloutKind::Todo,
        CalloutKind::Tip,
        CalloutKind::Success,
        CalloutKind::Question,
        CalloutKind::Warning,
        CalloutKind::Failure,
        CalloutKind::Danger,
        CalloutKind::Bug,
        CalloutKind::Example,
        CalloutKind::Quote,
    ];

    /// Obsidian's names and aliases; unknown kinds read as notes.
    pub fn parse(name: &str) -> CalloutKind {
        match name.to_ascii_lowercase().as_str() {
            "abstract" | "summary" | "tldr" => CalloutKind::Abstract,
            "info" => CalloutKind::Info,
            "todo" => CalloutKind::Todo,
            "tip" | "hint" | "important" => CalloutKind::Tip,
            "success" | "check" | "done" => CalloutKind::Success,
            "question" | "help" | "faq" => CalloutKind::Question,
            "warning" | "caution" | "attention" => CalloutKind::Warning,
            "failure" | "fail" | "missing" => CalloutKind::Failure,
            "danger" | "error" => CalloutKind::Danger,
            "bug" => CalloutKind::Bug,
            "example" => CalloutKind::Example,
            "quote" | "cite" => CalloutKind::Quote,
            _ => CalloutKind::Note,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CalloutKind::Note => "Note",
            CalloutKind::Abstract => "Abstract",
            CalloutKind::Info => "Info",
            CalloutKind::Todo => "Todo",
            CalloutKind::Tip => "Tip",
            CalloutKind::Success => "Success",
            CalloutKind::Question => "Question",
            CalloutKind::Warning => "Warning",
            CalloutKind::Failure => "Failure",
            CalloutKind::Danger => "Danger",
            CalloutKind::Bug => "Bug",
            CalloutKind::Example => "Example",
            CalloutKind::Quote => "Quote",
        }
    }
}
