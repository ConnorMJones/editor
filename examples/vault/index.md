---
tags: [example, showcase]
---
# Example vault

A tour of the markdown the editor renders. Open the rendered view with
`space v` and follow links with `gf`.

## Text

Plain text with *emphasis*, **strong**, ~~strikethrough~~ and `inline code`.
Mixed: ***strong emphasis*** and **bold with `code`**.

### Lists

1. First ordered item
2. Second ordered item
   1. Nested ordered item
   2. Another nested item
3. Third ordered item

- Unordered item
- Another one
  - Nested unordered
    - Deeper still
- Back at the top

#### Tasks

- [x] Write the example vault
- [x] Add a typst document
- [ ] Write the [[not-yet-written]] note
- [ ] Export everything to PDF

## Table

| Feature      | Format   |  Status |
|:-------------|:--------:|--------:|
| Markdown     | `.md`    |    done |
| Typst        | `.typ`   |    done |
| LaTeX math   | dollars  |    done |
| Mermaid      | fences   | partial |

## Quotes and notes

> A block quote. It can span
> several lines.

> [!note]
> Callouts use the Obsidian syntax.

> [!warning]- Folded title
> This body starts folded.

> [!tip]+
> This one is foldable and starts open.

Markdown also has footnotes.[^why]

[^why]: Footnotes render at the end of the note.

---

## Code

```rust
fn main() {
    let notes = ["index", "math", "diagrams"];
    for note in notes {
        println!("{note}.md");
    }
}
```

```python
def fib(n: int) -> int:
    return n if n < 2 else fib(n - 1) + fib(n - 2)
```

```toml
[package]
name = "example"
version = "0.1.0"
```

## Image

![A small gradient](assets/gradient.png)

## Links

- Wikilink: [[math]]
- With an alias: [[math|the math note]]
- To a heading: [[math#Matrices]]
- Diagrams: [[diagrams]]
- In a folder: [[notes/daily]]
- Markdown link: [the diagrams note](diagrams.md)
- URL: <https://typst.app> and [Obsidian](https://obsidian.md)

## Embed

![[math]]
