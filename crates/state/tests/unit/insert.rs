use crate::harness::{check, check_in};

#[test]
fn insert_and_append_cursor() {
    check("#[\n|]#", "i", "#[|\n]#");
    check("#[\n|]#", "i<esc>", "#[|\n]#");
    check("#[f|]#oo\n", "vll<A-;><esc>", "#[|foo]#\n");
    check("#[f|]#oo\n#(b|)#ar", "a", "#[fo|]#o\n#(ba|)#r");
    check("#[f|]#oo\n#(b|)#ar", "a<esc>", "#[f|]#oo\n#(b|)#ar");
    check("#[foo|]#", "abar<esc>", "#[foobar|]#\n");
    check("#[|foo]#", "abar<esc>", "#[foobar|]#\n");
}

#[test]
fn insert_at_line_bounds() {
    check("  a#[b|]#c\n", "Ix<esc>", "  x#[a|]#bc\n");
    check("  a#[b|]#c\n", "Ax<esc>", "  abcx#[\n|]#");
}

#[test]
fn open_lines() {
    check("Helix #[is|]# cool", "O", "#[\n|]#\nHelix is cool\n");
    check("  two spaces#[\n|]#", "Oa", "  a#[\n|]#  two spaces\n");
    check("I use\nb#[t|]#w.\n", "Oarch", "I use\narch#[\n|]#btw.\n");
    check("a#[b|]#c\n", "ox", "abc\nx#[\n|]#");
    check("a#[b|]#c\n", "2ox", "abc\nx#[\n|]#x#(\n|)#");
}

#[test]
fn newline_and_indent() {
    check(
        "#(|o)#ne\n#(|t)#wo\n#[|t]#hree\n",
        "i<ret>",
        "\n#(|o)#ne\n\n#(|t)#wo\n\n#[|t]#hree\n",
    );
    check(
        "hello       #[|\n]#world\n",
        "i<ret>",
        "hello\n#[|\n]#world\n",
    );
    check("hello        #[|w]#orld\n", "i<ret>", "hello\n#[|w]#orld\n");
    check("    foo#[|\n]#", "i<ret>", "    foo\n    #[|\n]#");
    check_in(
        "t.rs",
        "fn main() {#[|}]#\n",
        "i<ret>",
        "fn main() {\n    #[|\n]#}\n",
    );
    check_in(
        "t.rs",
        "// Hello world!#[|\n]#",
        "i<ret>",
        "// Hello world!\n// #[|\n]#",
    );
}

#[test]
fn tree_sitter_indent() {
    check_in(
        "t.rs",
        "fn f() {\n    if x {#[|\n]#    }\n}\n",
        "i<ret>",
        "fn f() {\n    if x {\n        #[|\n]#    }\n}\n",
    );
}

#[test]
fn leaving_insert_trims_blank_line() {
    check("    foo#[|\n]#", "i<ret><esc>", "    foo\n#[|\n]#");
    check("    foo#[|\n]#", "i<ret>x<esc>", "    foo\n    x#[|\n]#");
}

#[test]
fn auto_pairs() {
    check("#[|\n]#", "i(", "(#[|)]#\n");
    check("#[|\n]#", "i()", "()#[|\n]#");
    check("#[|\n]#", "i(<bs>", "#[|\n]#");
    check("#[|\n]#", "i\"", "\"#[|\"]#\n");
    check("#[|a]#", "i(", "(#[|a]#");
}

#[test]
fn insert_deletions() {
    check("hello world#[|\n]#", "i<C-w>", "hello #[|\n]#");
    check("hello world#[|\n]#", "i<A-backspace>", "hello #[|\n]#");
    check("  hello#[|\n]#", "i<C-u>", "  #[|\n]#");
    check("hel#[|l]#o", "i<C-h>", "he#[|l]#o");
    check("hel#[|l]#o", "i<C-d>", "hel#[o|]#");
    check("hel#[|l]#o", "i<del>", "hel#[o|]#");
    check("#[|hello]# world", "i<A-d>", "#[ |]#world");
    check("        #[|\n]#", "i<bs>", "    #[|\n]#");
    check(
        "#[abc|]#def\n#(abc|)#ef\n#(abc|)#f\n#(abc|)#\n",
        "a<del><esc>",
        "#[abc|]#ef\n#(abc|)#f\n#(abc|)#\n#(abc|)#\n",
    );
}

#[test]
fn insert_newline_variants() {
    check("a#[|b]#", "i<C-j>", "a\n#[|b]#");
    check("#[|a]#", "i<tab>", "\t#[|a]#");
    check("#[|a]#", "i<S-tab>", "\t#[|a]#");
}

#[test]
fn insert_register() {
    check("#[ab|]#\n", "yA<C-r>\"<esc>", "abab#[\n|]#");
}
