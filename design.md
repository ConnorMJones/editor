# Designing with types

How code in this repo gets written. AGENTS.md says *what* the code should look like; this
file says *how to get there*, in what order. The order is the point: types first, control
flow last. Starting from the types makes the rules in AGENTS.md come out of the design
rather than being applied to it afterwards.

## Why

The compiler checks exactly what the types say. Anything the types leave unsaid ("this
index is valid", "this pane is never empty", "this f32 is pixels") has to be checked by
hand, by every reader, at every change. So the work is to move facts into the types until
wrong programs fail to compile or fail at the boundary, as early as possible.

This is the Lean habit carried into Rust: a value of a type is evidence that something
holds. We can't prove everything in Rust, but we can make wrong states impossible to
construct and make every match handle every case.

## Workflow

Do these in order. The types are finished when the bodies can be written as plain matches
over them; until then, the work is still in step 1 or 2.

1. **Name the states.** Ask what this thing can be, and write each answer as an enum
   variant carrying exactly the data valid in that state. Then ask which combinations
   must never happen, and shape the types so those combinations can't be written.
2. **Name the owners.** For each invariant, choose the one function that performs the
   transitions able to break it. The fields it protects are private; everything else is
   public.
3. **Write the skeleton.** Types and signatures with `todo!()` bodies, then
   `cargo check`. Each function is total: every input has a defined result, and a failure
   is a typed error returned at once (fail early).
4. **Fill in the bodies.** Each body matches on the state it receives and builds the next
   one. When a body needs a fact the types don't carry, the fact goes into the types (step 1)
   before the body continues.
5. **Record the decision.** The type sketch and its reason go into the plan file, so a
   compaction or handoff carries the design itself.

**Bug fixes start at step 1** with two questions: which invariant was missing, and who
should own it? The type change answers both; the code change follows from it.

## Design questions

Ask these during step 1. Each answer is a type.

| Question | The answer becomes |
|---|---|
| What can this be, and what is known in each case? | an enum whose variants hold that knowledge |
| Which pieces of data must always agree? | one struct or enum holding them together |
| Is this value only meaningful in some states? | a field of those variants only |
| What proves this check passed? | the value the check returns: `Option<&TextDoc>`, `Result<T, E>` |
| Who is allowed to change this? | the owner function; private fields behind it |
| Where does this number come from? | the measurement, passed in as a typed unit (`Px`, `Pt`) |
| What are the kinds of this thing? | an enum, or a newtype whose constructor rejects bad input |
| Is the order of steps fixed when the code is written? | typestate if so, an enum if it changes at run time |
| Which other code reads this the same way? | one accessor, shared by all of them |

## Lean habits, in Rust

| Lean | Rust here |
|---|---|
| Inductive type whose constructors carry their data | `enum` whose variants hold exactly what is valid in that state |
| Structure carrying a proof | Newtype with private field and a checked constructor (`Flex::new -> Option<Flex>`): holding one is the proof |
| Totality | Exhaustive `match` naming every variant of our own enums, so a new variant reaches every place that must decide |
| `Decidable` with a witness | A check returns the evidence it found |
| Dependent indices | An index lives inside the structure it indexes (`Tabs` owns `active` and `preview`) |
| Lemmas | Helpers earn their place by being used in many proofs; a one-off step stays inline |
| Units as types | `Px`, `Pt`, `DevicePixels` |
| State machine | Typestate for a fixed order of steps; an enum for run-time transitions |

## Examples from this repo

**Scroll of a rendered view.** What can a rendered view's position be? A source line to
show once the text is parsed; a place the host reported; or a place keys asked to move
from, by a distance only the host can measure. Each case carries what is known in it:

```rust
pub enum Scroll {
    Line(usize),
    At(BlockScroll),
    Moved { from: BlockScroll, by: Motion },
}
```

The state never holds pixel guesses; the host measures `Moved` and reports `At`.

**List sync in the host.** What does the host's cached scroll know? Either where the list
was last set, or that items moved under it since: `enum Applied { Stale, At(BlockScroll) }`.

**Closing tabs.** Which invariant governs panes? A pane with no tabs exists only as the last
one. Who owns it? `Layout::remove_tab`, the one path that removes a tab (`Tabs::remove` is
private), closes a pane left empty.

**Preview tabs.** What is known about the preview tab? Its index and how it came to be, and
the two always travel together:

```rust
preview: Option<Preview>,   // Preview { index, kind: PreviewKind::{Open, SplitCopy} }
```

**Checks that return their evidence.** What does "this buffer can be rendered" prove? That
its text is loaded and its format known, so the check returns the text:

```rust
let Some(doc) = renderable(state, buffer) else { return };
```

## Functions as transformations

A function takes values that carry what is known and builds the next value. It doesn't
interrogate the state to find out what it was given. Four habits keep it that way.

- **An operation returns what it produced.** Showing a tab returns the view it showed;
  closing a pane returns what it removed. The caller continues from that value instead of
  looking it up again and checking that the operation did what it just did.
- **Inputs carry their evidence.** An event or command carries what its producer knew: a
  click on a drawn block carries the line the block was drawn from, not a bare index that
  the receiver has to resolve again. The one case that can really go stale, such as a pane
  closed since the last frame, is checked once, at the entry point, and named there.
- **A repeated check means a missing owner.** When several call sites guard the same
  condition, the condition is an invariant nobody owns. Give it an owner that keeps it true
  (closing a pane unlinks the views following it), and the guards have nothing left to check.
- **Change code from its owner outward.** When touching existing code, start from the type
  or function that owns the data, change it, and let the compiler list the callers. Starting
  from the call site in front of you leads to patching that site's control flow.

### Example: clicking a rendered block

Before, the function rediscovers everything the host already knew, and re-checks what it
had just made true:

```rust
pub fn click(state: &mut State, pane: PaneId, block: usize) {
    let Some(view) = state.layout.rendered(pane) else { return };
    let Some(item) = view.scroll.anchor().map(|at| at.item) else { return };
    let Some(doc) = state.buffers[buffer].doc() else { return };
    let Some(line) = markdown(..).and_then(|m| m.blocks.get(block)).map(..) else { return };
    let at = rope.line_to_char(line.min(rope.len_lines() - 1));
    match link {
        Link::Follows { pane: source, seen } if state.layout.panes().contains_key(source) => {
            panes::show(state, source, TabKey::Text(buffer), OpenAs::Permanent);
            match state.layout.pane_mut(source).and_then(Pane::active_mut) {
                Some(View::Text(text)) if text.buffer() == buffer => text.selection = ..,
                Some(_) | None => {}
            }
            state.layout.set_focus(source).expect("the source pane was just shown");
            // ... looks the pane and the document up again, then the rendered view again
        }
        Link::Follows { .. } | Link::Standalone => { .. }
    }
}
```

Each guard answers a question the types could have answered:

- The block, the document and the parse exist because the host drew the block from them,
  so the click carries the block's line.
- `contains_key(source)` holds once closing a pane unlinks its followers.
- The view lookup after `show` goes away once `show` returns the view.
- The clamp covers a parse that doesn't match the text, which `markdown()` already rules out.

After, there is one boundary check and one match on the state that matters:

```rust
pub fn click(state: &mut State, pane: PaneId, line: usize) {
    let Some(view) = state.layout.rendered(pane) else { return }; // closed since drawn
    let (buffer, link) = (view.text.buffer(), view.link);
    match link {
        Link::Follows { pane: source, seen } => {
            let text = panes::show_text(state, source, buffer);
            text.selection = Selection::point(..);
            state.layout.focus(source);
            scroll::follow_cursor(state, source);
        }
        Link::Standalone => { .. }
    }
}
```

### When you catch yourself writing

Each of these means a design question was skipped. Go back and answer it; the code that
follows is usually shorter.

| You're about to write | Ask |
|---|---|
| `let … else { return }` on something you just established | Should the operation that established it return it? |
| `.expect()` right after an operation | Should the owner do both steps as one operation? |
| `None => {}` or `_ => {}` for a case that can't happen | Which type makes the case unrepresentable? |
| `match (bool, bool)`, or `match cond { true => .., false => .. }` | Which enum is this? |
| A clamp, a default or `.ok()` so the code can keep going | Where should this fail, when the value is built? |
| The same guard at several call sites | Which owner keeps this true? |
| A function returning `()` that callers follow with a lookup | What should it return? |

## Changing many call sites

Cleanups touch many sites at once, and a mechanical rewrite keeps whatever shape the old
code had. So:

- **Rename mechanically, redesign by hand.** Scripts are for changes whose point is to keep
  the shape: moving files, renaming paths. A change in meaning goes through the owner, and
  the compiler errors become the list of sites to revisit.
- **Do one site by hand first.** Write the first caller in the new shape and look at it.
  If it is no shorter than before, the owner's change isn't finished yet.
- **Sort the sites before editing them.** List the call sites and group them by what they
  actually need. Each group gets one answer (delete the check, use the returned value,
  check at the boundary). Sites aren't edited one by one in the order they appear.
- **Expect deletions.** A cleanup that moves facts into types removes guards. A diff where
  every site keeps its branches, rewritten, has moved the problem rather than solved it.
- **One invariant per change.** Each change is small enough to read whole before the next
  one starts.

## Agents and handoffs

- A brief gives the agent the type skeleton (enums, signatures, the owner of each
  invariant) to implement inside, and points to this file; the skeleton carries the design.
- Design decisions go into the plan file as they are made.
- The end-of-branch review checks architecture. It doesn't replace the design step,
  because a review can only flag what was built, not change the order it was built in.
