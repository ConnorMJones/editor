#import "lib.typ": note

= An included chapter

This page comes from `chapter.typ`, pulled in with `#include`. Edits to it
show up in the preview of `main.typ` on the next compile.

#note[Included][Imports and includes resolve relative to the project root.]

== Code

```rust
fn area(radius: f64) -> f64 {
    std::f64::consts::PI * radius * radius
}
```
