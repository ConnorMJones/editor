use super::{Context, tables};

const CUSTOM: &str = "\
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
";

const SOURCE_VIEW: &str = "\
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
";

const MARKDOWN_VIEW: &str = "\
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
";

const DRAWINGS: &str = "\
## Drawings, images, PDFs and books

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
| `+` / `-` / `=` | pages, drawing, image, PDF | Zoom in / out / fit |
| `zf` | pages, drawing, image, PDF | Fit width / height |
| `h j k l`, arrows | pages, drawing, image, PDF | Pan |
| `:export-pdf` / `:export-svg` / `:export-png` | drawing | Export |
| `v` | PDF, book | Highlight |
| `i` | PDF, book | Highlight with a comment / edit the comment |
| `I` | PDF, book | Write the comment beside the page |
| `C-v` | comment prompt | Move to the full view |
| `esc` | comment | Save and close |
| `C-s`, `:w` | comment | Save |
| `space q` | comment | Close without saving |
| `space v` | comment | Full / split |
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
";

pub fn reference() -> String {
    let mut out = String::from(
        "# Keymap\n\nGenerated from `crates/state/src/keymap.rs`; regenerate with \
         `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.\n\n",
    );
    out.push_str(CUSTOM);
    out.push_str("\n");
    out.push_str(SOURCE_VIEW);
    out.push_str("\n");
    out.push_str(MARKDOWN_VIEW);
    out.push_str("\n");
    out.push_str(DRAWINGS);
    out.push_str("\n## Typed commands (`:`)\n\n| Names |\n| --- |\n");
    for (names, _) in crate::input::prompt::COMMANDS {
        let names: Vec<String> = names.iter().map(|name| format!("`:{name}`")).collect();
        out.push_str(&format!("| {} |\n", names.join(", ")));
    }
    for context in Context::ALL {
        out.push_str(&format!(
            "\n## {context:?}\n\n| Keys | Command |\n| --- | --- |\n"
        ));
        for (keys, command) in tables(context).iter().copied().flatten() {
            let keys: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
            out.push_str(&format!("| `{}` | `{command:?}` |\n", keys.join(" ")));
        }
    }
    out
}
