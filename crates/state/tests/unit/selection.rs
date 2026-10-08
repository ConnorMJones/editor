use crate::harness::check;

#[test]
fn collapse_flip_and_primary() {
    check("#[hel|]#lo", ";", "he#[l|]#lo");
    check("#[hel|]#lo", "<A-;>", "#[|hel]#lo");
    check("#[|hel]#lo", "<A-:>", "#[hel|]#lo");
    check("#[a|]# #(b|)# #(c|)#", ",", "#[a|]# b c");
    check("#[a|]# #(b|)# #(c|)#", "<A-,>", "a #[b|]# #(c|)#");
    check("#[a|]# #(b|)# #(c|)#", ")", "#(a|)# #[b|]# #(c|)#");
    check("#[a|]# #(b|)# #(c|)#", "(", "#(a|)# #(b|)# #[c|]#");
}

#[test]
fn line_selection() {
    check(
        "#[l|]#orem\nipsum\ndolor\n",
        "x",
        "#[lorem\n|]#ipsum\ndolor\n",
    );
    check(
        "#[l|]#orem\nipsum\ndolor\n",
        "xx",
        "#[lorem\nipsum\n|]#dolor\n",
    );
    check(
        "#[l|]#orem\nipsum\ndolor\n",
        "x2x",
        "#[lorem\nipsum\ndolor\n|]#",
    );
    check("#[l|]#orem\nipsum\n", "2x", "#[lorem\nipsum\n|]#");
    check("lo#[rem\nip|]#sum\n", "X", "#[lorem\nipsum\n|]#");
    check(
        "lo#[rem\nipsum\ndo|]#lor\n",
        "<A-x>",
        "lorem\n#[ipsum\n|]#dolor\n",
    );
    check("#[a|]#\nb", "%", "#[a\nb\n|]#");
}

#[test]
fn split_and_merge() {
    check("#[a\nb\n|]#", "<A-s>", "#[a|]#\n#(b|)#\n");
    check("#[a|]##(b|)# c", "<A-minus>", "#[ab|]# c");
    check("#[a|]##(b|)# #(c|)#", "<A-_>", "#[ab|]# #(c|)#");
    check("#[a,b,c|]#", "S,<ret>", "#[a|]#,#(b|)#,#(c|)#");
    check("#[one two one|]#", "sone<ret>", "#[one|]# two #(one|)#");
    check("#[a|]# #(bb|)# #(c|)#", "Kb<ret>", "a #[bb|]# c");
    check("#[a|]# #(bb|)# #(c|)#", "<A-K>b<ret>", "#[a|]# bb #(c|)#");
    check("#[one two|]#", "sxyz<ret>", "#[one two|]#");
}

#[test]
fn copy_selection_on_line() {
    check(
        "#[lo|]#rem\nipsum\ndolor\n",
        "CC",
        "#(lo|)#rem\n#(ip|)#sum\n#[do|]#lor\n",
    );
    check(
        "#[|lo]#rem\nipsum\ndolor\n",
        "CC",
        "#(|lo)#rem\n#(|ip)#sum\n#[|do]#lor\n",
    );
    check("test\n#[testitem|]#\n", "<A-C>", "test\n#[testitem|]#\n");
    check("test\n#[test|]#\n", "<A-C>", "#[test|]#\n#(test|)#\n");
    check("#[testitem|]#\ntest\n", "C", "#[testitem|]#\ntest\n");
}

#[test]
fn trim_and_align() {
    check("#[  a  |]#b", "_", "  #[a|]#  b");
    check(
        "#[a|]# #(bbb|)#\n#(cc|)# #(d|)#",
        "&",
        " #[a|]# #(bbb|)#\n#(cc|)#   #(d|)#",
    );
}

#[test]
fn rotate_contents() {
    check("#[a|]# #(b|)# #(c|)#", "<A-)>", "#(c|)# #[a|]# #(b|)#");
    check("#[a|]# #(b|)# #(c|)#", "<A-(>", "#(b|)# #(c|)# #[a|]#");
}
