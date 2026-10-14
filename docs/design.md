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

## Agents and handoffs

- A brief gives the agent the type skeleton (enums, signatures, the owner of each
  invariant) to implement inside, and points to this file; the skeleton carries the design.
- Design decisions go into the plan file as they are made.
- The end-of-branch review checks architecture. It doesn't replace the design step,
  because a review can only flag what was built, not change the order it was built in.
