use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use helix_core::Rope;
use state::project::links::Index;
use state::render::markdown::{self, Base};

/// One section of a typical note, about 50 lines.
const SECTION: &str = "\
## Section heading

A paragraph with *emphasis*, **strong text**, `inline code`, a [link](https://example.com)
and a [[Wiki Link|alias]] spanning a couple of lines so the soft break is exercised, plus
some inline math $e^{i\\pi} + 1 = 0$ for good measure.

- [ ] an open task with **bold**
- [x] a finished task
- a plain item
  - nested item with `code`

> [!tip]- A folded callout
> with a body line
> and another one

```rust
fn main() {
    let x = 1;
    println!(\"{x}\");
}
```

| Column | Other | Third |
|:-------|:-----:|------:|
| a      | b     | c     |
| d      | e     | f     |

$$
\\int_0^1 x^2 \\, dx
$$

1. first
2. second
3. third

Another paragraph that is fairly long and keeps going so that the text has a realistic
density of prose between the structural elements, as most notes are mostly prose.
More prose here with ~~struck~~ words and a footnote reference.[^n]

---

";

fn markdown(c: &mut Criterion) {
    let loader = helix_core::config::default_lang_loader();
    loader.set_scopes(
        model::theme::SCOPES
            .iter()
            .map(|(scope, _)| (*scope).to_owned())
            .collect(),
    );
    let repeats = 5000 / SECTION.lines().count() + 1;
    let source = SECTION.repeat(repeats);
    let text = Rope::from(source.as_str());
    let vault = Index::new(1, Default::default());
    let base = Base {
        dir: Path::new("/nonexistent"),
        root: Path::new("/nonexistent"),
        folder: "",
        vault: &vault,
    };
    let mut parsed = markdown::parse(text.slice(..), base, None);
    markdown::highlight(&mut parsed, &loader);
    println!(
        "{} lines, {} KB, {} blocks, {} top-level",
        text.len_lines(),
        text.len_bytes() / 1024,
        parsed.blocks.len(),
        parsed.roots.len()
    );
    let mut reuse = Some(parsed);
    c.bench_function("parse 5k-line note", |b| {
        b.iter(|| {
            let mut parsed = markdown::parse(black_box(text.slice(..)), base, reuse.take());
            markdown::highlight(&mut parsed, &loader);
            let blocks = parsed.blocks.len();
            reuse = Some(parsed);
            blocks
        })
    });
    c.bench_function("parse 5k-line note, cold highlight cache", |b| {
        b.iter(|| {
            let mut parsed = markdown::parse(black_box(text.slice(..)), base, None);
            markdown::highlight(&mut parsed, &loader);
            parsed.blocks.len()
        })
    });
}

criterion_group!(benches, markdown);
criterion_main!(benches);
