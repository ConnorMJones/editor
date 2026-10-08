use model::mode::Mode;

use crate::harness::{Harness, check, check_in};

#[test]
fn delete_and_change() {
    check("#[hel|]#lo", "d", "#[l|]#o");
    check("#[hel|]#lo", "<A-d>", "#[l|]#o");
    check("#[hel|]#lo", "cy<esc>", "y#[l|]#o");
    check("#[l|]#orem\nipsum\n", "xd", "#[i|]#psum\n");
    check(
        "#[fn main() {}\n|]#// a comment\n",
        "c",
        "#[\n|]#// a comment\n",
    );
    check("#[a|]# b", "vld", "#[b|]#");
}

#[test]
fn yank_and_paste() {
    check(
        "#[|lorem]#\n#(|ipsum)#\n#(|dolor)#\n",
        "yp",
        "lorem#[|lorem]#\nipsum#(|ipsum)#\ndolor#(|dolor)#\n",
    );
    check("#[ab|]#c", "yP", "#[ab|]#abc");
    check("#[ab|]#c", "y2p", "ab#[abab|]#c");
    check("#[a|]#\nb\n", "xyp", "a\n#[a\n|]#b\n");
    check("#[a|]#\nb\n", "xyjP", "a\n#[a\n|]#b\n");
    check("#[ab|]# cd", "ywR", "a#[ab|]#cd");
    check("#[ab|]# cd", "\"ayw\"ap", "ab #[ab|]#cd");
    check("#[ab|]# cd", "\"aywp", "a#[b |]#cd");
    check("#[ab|]# cd", "\"_dP", "#[ |]#cd");
}

#[test]
fn special_registers() {
    check("#[a|]# #(b|)#", "\"#R", "#[1|]# #(2|)#");
    check("#[a|]# #(b|)#", "\".p", "a#[a|]# b#(b|)#");
    let mut h = Harness::with("name.txt", "#[x|]#\n");
    h.keys("\"%p");
    assert_eq!(h.plain(), "x#[name.txt|]#\n");
}

#[test]
fn replace_char() {
    check("#[hel|]#lo", "rx", "#[xxx|]#lo");
    check("#[a|]#b", "r<ret>", "#[\n|]#b");
    check("#[ab|]#", "vr-", "#[--|]#");
}

#[test]
fn case_changes() {
    check("#[aBc|]#d", "~", "#[AbC|]#d");
    check("#[aBc|]#d", "`", "#[abc|]#d");
    check("#[aBc|]#d", "<A-`>", "#[ABC|]#d");
}

#[test]
fn indent_unindent() {
    check("#[a|]#\nb", "><", "#[a|]#\nb");
    // A `\n` right after a selection ending in `\n` is skipped by helix notation, hence the double.
    check("#[a\nb|]#\n\nc", "x>", "\t#[a\n\tb\n|]#\n\nc");
    check("\t\t#[a|]#", "<", "\t#[a|]#");
    check("    #[a|]#", "<", "#[a|]#");
}

#[test]
fn join() {
    check("#[a|]#bc\ndef\n", "J", "#[a|]#bc def\n");
    check("#[a|]#bc\n    def\n", "J", "#[a|]#bc def\n");
    check("#[a|]#bc\n\ndef\n", "J", "#[a|]#bc\ndef\n");
    check(
        "#[a\nb\nc\nd\ne|]#\n",
        "<A-J>",
        "a#[ |]#b#( |)#c#( |)#d#( |)#e\n",
    );
    check("#[a|]#bc\ndef\n", "<A-J>", "abc#[ |]#def\n");
    check_in("t.rs", "/// #[a|]#bc\n/// def\n", "J", "/// #[a|]#bc def\n");
    check_in(
        "t.rs",
        "#[| // a\n// b\n/// c\n/// d\ne\n/// f\n// g]#\n",
        "J",
        "#[| // a b /// c d e f // g]#\n",
    );
}

#[test]
fn toggle_comments() {
    check_in("t.rs", "#[l|]#et x = 5;\n", "<C-c>", "// #[l|]#et x = 5;\n");
    check_in("t.rs", "// #[a|]#bc\n", "<C-c>", "#[a|]#bc\n");
    check_in("t.rs", "/// #[a|]#bc\n", "<C-c>", "#[a|]#bc\n");
    check("#[a|]#\n", "<C-c>", "# #[a|]#\n");
}

#[test]
fn surround() {
    check("#[a|]#", "ms(", "#[(a)|]#");
    check("#[ab|]#", "ms]", "#[[ab]|]#");
    check("(#[|a]#)\n", "mrm{", "{#[|a]#}\n");
    check("(#[a|]#)\n", "mr({", "{#[a|]#}\n");
    check("{{\n#(}|)#\n#[}|]#\n", "mrm)", "((\n#()|)#\n#[)|]#\n");
    check("(#[|a]#)\n", "mdm", "#[|a]#\n");
    check("[(#[a|]#)]\n", "md[", "(#[a|]#)\n");
    check("#[a|]#", "md(", "#[a|]#");
}

#[test]
fn increment_decrement() {
    check("#[1|]#", "<C-a>", "#[2|]#");
    check("#[9|]#", "<C-a>", "#[10|]#");
    check("#[1|]#", "5<C-x>", "#[-4|]#");
    check("#[1|]# #(1|)#", "\"#<C-a>", "#[2|]# #(3|)#");
}

#[test]
fn add_newlines() {
    check("#[a|]#", "]<space>", "#[a|]#\n\n");
    check("#[a|]#", "[<space>", "\n#[a|]#");
    check("#[a|]#", "2[<space>", "\n\n#[a|]#");
}

#[test]
fn undo_redo() {
    check("#[a|]#bc", "du", "#[a|]#bc");
    check("#[a|]#bc", "dduU", "#[c|]#");
    check("#[a|]#bc", "ddd2u", "#[b|]#c");
    check("#[a|]#bc", "d<A-u><A-U>", "#[b|]#c");
    check("#[\n|]#", "[<space>u[<space>u", "#[\n|]#");
}

#[test]
fn repeat_insert() {
    check("#[a|]#\nb\n", "ix<esc>j.", "xa\nbx#[|\n]#");
    check("#[a|]#\n", "Ax<esc>.", "axx#[\n|]#");
    check("#[a|]#\nb\n", "ox<esc>.", "a\nx\nx#[\n|]#b\n");
}

#[test]
fn counts() {
    let mut h = Harness::with("t.txt", "#[a|]#bcdef\n");
    h.keys("1");
    assert_eq!(h.state.input.count.map(|c| c.get()), Some(1));
    h.keys("2");
    assert_eq!(h.state.input.count.map(|c| c.get()), Some(12));
    h.keys("<esc>");
    assert_eq!(h.state.input.count, None);
    h.keys("3l");
    assert_eq!(h.plain(), "abc#[d|]#ef\n");
}

#[test]
fn modes() {
    let mut h = Harness::with("t.txt", "#[a|]#b\n");
    h.keys("v");
    assert_eq!(h.state.input.mode, Mode::Select);
    h.keys("v");
    assert_eq!(h.state.input.mode, Mode::Normal);
    h.keys("vd");
    assert_eq!(h.state.input.mode, Mode::Normal);
    h.keys("c");
    assert_eq!(h.state.input.mode, Mode::Insert);
}

#[test]
fn typed_commands() {
    check(
        "#[c|]#\n#(a|)#\n#(b|)#",
        ":sort<ret>",
        "#[a|]#\n#(b|)#\n#(c|)#",
    );
    check(
        "#[c|]#\n#(a|)#\n#(b|)#",
        ":sort -r<ret>",
        "#[c|]#\n#(b|)#\n#(a|)#",
    );
    check("#[o|]#ne\ntwo\nthree", ":3<ret>", "one\ntwo\n#[t|]#hree");
    check(
        "#[o|]#ne\ntwo\nthree",
        ":goto 2<ret>",
        "one\n#[t|]#wo\nthree",
    );
    check("#[aaa bbb ccc|]#", ":reflow 7<ret>", "#[aaa bbb\nccc|]#");
}
