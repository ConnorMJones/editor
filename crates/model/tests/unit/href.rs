use model::href::{Href, normalize, percent_decode, scheme};

#[test]
fn a_scheme_is_a_letter_then_letters_digits_or_plus_dot_dash() {
    assert_eq!(scheme("https://example.com"), Some("https"));
    assert_eq!(scheme("mailto:someone@example.com"), Some("mailto"));
    assert_eq!(scheme("svn+ssh://host"), Some("svn+ssh"));
    assert_eq!(scheme("c:/windows"), None, "a drive letter is not a scheme");
    assert_eq!(scheme("../notes/a.md"), None);
    assert_eq!(scheme("chapter.xhtml#a:b"), None);
}

#[test]
fn an_href_is_a_url_or_a_local_path_with_its_anchor() {
    assert_eq!(Href::parse("https://a.b/c#d"), Href::Url("https://a.b/c#d"));
    assert_eq!(
        Href::parse("two.xhtml#later"),
        Href::Local {
            path: "two.xhtml",
            anchor: Some("later")
        }
    );
    assert_eq!(
        Href::parse("#top"),
        Href::Local {
            path: "",
            anchor: Some("top")
        }
    );
}

#[test]
fn percent_escapes_decode_and_bad_ones_are_left_alone() {
    assert_eq!(percent_decode("b%20two.xhtml"), "b two.xhtml");
    assert_eq!(percent_decode("100%"), "100%");
    assert_eq!(percent_decode("%+1"), "%+1");
    assert_eq!(
        percent_decode("%ff"),
        "%ff",
        "not UTF-8, so the text is kept"
    );
}

#[test]
fn a_relative_path_cannot_climb_out_of_its_root() {
    assert_eq!(
        normalize("text", "../images/a.png"),
        Some(vec!["images", "a.png"])
    );
    assert_eq!(normalize("", "./a/./b"), Some(vec!["a", "b"]));
    assert_eq!(normalize("text", "../../a.png"), None);
}
