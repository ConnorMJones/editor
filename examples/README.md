# Examples

Files that show off the editor's rendering. Tests in `crates/typeset` and
`crates/state` check that every example still parses and renders.

| Folder   | Shows |
|----------|-------|
| `vault/` | An Obsidian-style vault: every markdown block, callouts, wikilinks, embeds, backlinks, LaTeX math (`math.md`) and mermaid diagrams (`diagrams.md`). |
| `typst/` | A multi-page typst document with an import, an include, math, a table and a figure. |
| `git/`   | `make-conflict.sh` builds a temp repo with staged, unstaged, untracked and deleted files and a merge conflict, then prints its path. |

LaTeX is supported only as math inside markdown (`$...$` and `$$...$$`);
full LaTeX documents are not.

## Opening

```sh
editor examples/vault
editor examples/typst
editor "$(examples/git/make-conflict.sh)"
```

## Keys

| Key           | Does |
|---------------|------|
| `space v`     | Rendered preview in a split beside the text. |
| `space V`     | Toggle the tab between text and rendered view in place. |
| `gf`          | Follow the wikilink, markdown link or URL under the cursor. |
| `space l`     | Backlinks: notes linking to the focused note. |
| `:export-pdf` | Compile the focused typst file to a PDF beside it. |
