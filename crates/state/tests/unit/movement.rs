use crate::harness::{check, check_in};

#[test]
fn char_and_line_motions() {
    check("#[h|]#ello\nworld", "l", "h#[e|]#llo\nworld");
    check("he#[l|]#lo\nworld", "j", "hello\nwo#[r|]#ld");
    check("hello\nwo#[r|]#ld", "k", "he#[l|]#lo\nworld");
    check("he#[l|]#lo\nworld", "h", "h#[e|]#llo\nworld");
    check("#[h|]#ello world", "3l", "hel#[l|]#o world");
    check("#[h|]#ello\nworld", "vll", "#[hel|]#lo\nworld");
}

#[test]
fn word_motions() {
    check("#[h|]#ello world", "w", "#[hello |]#world");
    check("#[h|]#ello world foo", "2w", "hello #[world |]#foo");
    check("hello #[w|]#orld", "b", "#[|hello ]#world");
    check("#[h|]#ello world", "e", "#[hello|]# world");
    check("#[a|]#.b c", "W", "#[a.b |]#c");
    check("#[a|]#.b c", "E", "#[a.b|]# c");
    check("a.b #[c|]#", "B", "#[|a.b ]#c");
    check("#[h|]#ello world foo", "vww", "#[hello world |]#foo");
}

#[test]
fn goto_mode() {
    check("one\ntwo\nthr#[e|]#e", "gg", "#[o|]#ne\ntwo\nthree");
    check("#[o|]#ne\ntwo\nthree", "ge", "one\ntwo\n#[t|]#hree");
    check("#[o|]#ne\ntwo\nthree", "2gg", "one\n#[t|]#wo\nthree");
    check("#[o|]#ne\ntwo\nthree", "3G", "one\ntwo\n#[t|]#hree");
    check("#[o|]#ne\ntwo\nthree", "G", "#[o|]#ne\ntwo\nthree");
    check("  he#[l|]#lo", "gh", "#[ |]# hello");
    check("  he#[l|]#lo", "gl", "  hell#[o|]#");
    check("  he#[l|]#lo", "gs", "  #[h|]#ello");
    check("#[h|]#ello", "3g|", "he#[l|]#lo");
    check("  he#[l|]#lo", "<home>", "#[ |]# hello");
    check("  he#[l|]#lo", "<end>", "  hell#[o|]#");
}

#[test]
fn paragraphs() {
    check("#[a|]#\nb\n\nc\n", "]p", "#[a\nb\n\n|]#c\n");
    check("a\nb\n\n#[c|]#\n", "[p", "#[|a\nb\n\n]#c\n");
}

#[test]
fn find_char() {
    check("he#[l|]#lo\nhello", "fl", "he#[ll|]#o\nhello");
    check("hel#[l|]#o\nhello", "fl", "hel#[lo\nhel|]#lo");
    check("hel#[l|]#o\nhello", "fx", "hel#[l|]#o\nhello");
    check("he#[l|]#lo\nhello", "2fl", "he#[llo\nhel|]#lo");
    check("#[h|]#ello\nhello", "9fl", "#[h|]#ello\nhello");
    check("h#[e|]#llo\nhello", "tl", "h#[el|]#lo\nhello");
    check("he#[l|]#lo\nhello", "tl", "he#[llo\nhe|]#llo");
    check("hello\nhel#[l|]#o", "Fl", "hello\nhe#[|ll]#o");
    check("hello\nhe#[l|]#lo", "Fl", "hel#[|lo\nhel]#lo");
    check("hello\nhel#[l|]#o", "2Fl", "hel#[|lo\nhell]#o");
    check("hello\nhell#[o|]#", "Tl", "hello\nhel#[|lo]#");
    check("hello\nhe#[l|]#lo", "Tl", "hell#[|o\nhel]#lo");
    check("hello\nhel#[l|]#o", "2Tl", "hel#[|lo\nhell]#o");
}

#[test]
fn find_char_line_ending() {
    check("on#[e|]#\ntwo\n", "f<ret>", "on#[e\n|]#two\n");
    check("one#[\n|]#two\n", "f<ret>", "one#[\ntwo\n|]#");
    check("#[o|]#ne\ntwo\n", "2f<ret>", "#[one\ntwo\n|]#");
    check("o#[n|]#e\ntwo\n", "t<ret>", "o#[ne|]#\ntwo\n");
    check("one\ntwo\n#[t|]#hree\n", "F<ret>", "one\ntwo#[|\nt]#hree\n");
    check("one\ntwo\nth#[r|]#ee\n", "T<ret>", "one\ntwo\n#[|thr]#ee\n");
    check(
        "one\n#[|t]#wo\nthree",
        "T<ret>gll2f<ret>",
        "one\ntwo#[\n|]#three",
    );
}

#[test]
fn repeat_last_motion() {
    check("#[o|]#ne two\none two", "ft<A-.>", "one #[two\none t|]#wo");
    check(
        "#[o|]#ne two\none two\n",
        "ftf<ret><A-.>",
        "one two#[\none two\n|]#",
    );
}

#[test]
fn text_objects() {
    check(
        "(so [many {go#[o|]#d} text] here)",
        "mi{",
        "(so [many {#[good|]#} text] here)",
    );
    check(
        "(so [many {go#[o|]#d} text] here)",
        "mi[",
        "(so [#[many {good} text|]#] here)",
    );
    check(
        "'so 'many 'go#[o|]#d' text' here'",
        "2mi'",
        "'so '#[many 'good' text|]#' here'",
    );
    check(
        "(so [many {go#[|od]#} text] here)",
        "mi{",
        "(so [many {#[|good]#} text] here)",
    );
    check("some (#[t|]#ext) here", "mim", "some (#[text|]#) here");
    check(
        "(so (many (go#[o|]#d) text) here)",
        "2mim",
        "(so (#[many (good) text|]#) here)",
    );
    check("some (#[t|]#ext) here", "mam", "some #[(text)|]# here");
    check(
        "(so (many (go#[o|]#d) text) here)",
        "3mam",
        "#[(so (many (good) text) here)|]#",
    );
    check("hello wo#[r|]#ld foo", "miw", "hello #[world|]# foo");
    check("hello wo#[r|]#ld foo", "maw", "hello #[world |]#foo");
    check("a\n#[b|]#\n\nc", "map", "#[a\nb\n\n|]#c");
}

#[test]
fn match_brackets() {
    check("#[(|]#a b)", "mm", "(a b#[)|]#");
    check("(a b#[)|]#", "mm", "#[(|]#a b)");
    check("#[(|]#a b)", "vmm", "#[(a b)|]#");
}

#[test]
fn syntax_objects() {
    let rust = "#[/|]#// Increments\nfn inc(x: usize) -> usize { x + 1 }\n/// Decrements\nfn dec(x: usize) -> usize { x - 1 }\n";
    check_in(
        "foo.rs",
        rust,
        "]fv]f",
        "/// Increments\n#[fn inc(x: usize) -> usize { x + 1 }\n/// Decrements\nfn dec(x: usize) -> usize { x - 1 }|]#\n",
    );
    check_in(
        "foo.rs",
        "fn foo() {\n    let result = if true {\n        \"yes\"\n    } else {\n        \"no#[\"|]#\n    }\n}\n",
        "<A-e>",
        "fn foo() {\n    let result = if true {\n        \"yes\"\n    } else {\n        \"no\"#[\n|]#    }\n}\n",
    );
}

#[test]
fn search() {
    check("#[o|]#ne two one two", "/two<ret>", "one #[two|]# one two");
    check("#[o|]#ne two one two", "/two<ret>n", "one two one #[two|]#");
    check(
        "#[o|]#ne two one two",
        "/two<ret>nn",
        "one #[two|]# one two",
    );
    check("one two one #[t|]#wo", "?one<ret>", "one two #[one|]# two");
    check("one two one #[t|]#wo", "/one<ret>N", "one two #[one|]# two");
    check("#[o|]#ne two one two", "/TWO<ret>", "#[o|]#ne two one two");
    check("#[o|]#ne two one two", "/two<esc>", "#[o|]#ne two one two");
    check(
        "#[o|]#ne two one two",
        "/two<ret>vn",
        "one #(two|)# one #[two|]#",
    );
}

#[test]
fn search_selection() {
    check("#[one|]# two one", "*n", "one two #[one|]#");
    check("#[o|]#ne\ntwo\nthree", "gej*h", "one\ntwo\nthree#[\n|]#");
    check("#[on|]#e on", "*n", "one #[on|]#");
    check("#[on|]#e on", "<A-*>n", "one #[on|]#");
}

#[test]
fn goto_arrow_aliases() {
    check(
        "one\nt#[w|]#o\nthree\n",
        "g<right>",
        "one\ntw#[o|]#\nthree\n",
    );
    check(
        "one\nt#[w|]#o\nthree\n",
        "g<left>",
        "one\n#[t|]#wo\nthree\n",
    );
    check("one\nt#[w|]#o\nthree\n", "g<up>", "#[o|]#ne\ntwo\nthree\n");
    check(
        "one\nt#[w|]#o\nthree\n",
        "g<down>",
        "one\ntwo\n#[t|]#hree\n",
    );
}
