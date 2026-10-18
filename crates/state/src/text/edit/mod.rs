//! Commands that change the focused text.

mod delete;
mod insert;
mod lines;
mod rewrite;
mod surround;
mod yank;

pub use delete::{
    Operation, delete_char_backward, delete_char_forward, delete_insert_mode, delete_selection,
    delete_word_backward, delete_word_forward, kill_to_line_start,
};
pub use insert::{
    Fallback, append_mode, exit_insert, insert_char, insert_newline, insert_tab,
    insert_with_indent, open, smart_tab,
};
pub use lines::{
    add_newline, align_selections, indent, join_selections, reflow, sort, toggle_comments, unindent,
};
pub use rewrite::{change_case, increment, replace, rotate_contents};
pub use surround::{surround_add, surround_delete, surround_replace};
pub use yank::{Yank, paste, paste_from, replace_from, replace_with, yank};
