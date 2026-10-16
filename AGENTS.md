### Coding Guidelines
Before writing or changing code, read docs/design.md; it describes the approach, and this
file describes the result.

Code style:

Code Guidelines:
- Comments only for information/assumptions not apparent from the code. One
  short plain sentence. No history, no measurements, no justification, no
  doc comment on every function. Do not let this regress immediately after
  you check once.
- Write code such that invalid state irrepresentable (THIS IS VERY IMPORTANT).
  The type system is there for explicitly this purpose. It is a core aspect of
  the program which gives richer information to the logic being run.
  Code should be type oriented where the types convey intentionality and
  ensure correctness of the actual program. 
- `match` over if/else chains. You can use maps and functional paradigms.
  Avoid using bool flags and structure you're branching off of stricter
  type architecture. Similarly avoid overusing if let Some patterns. The
  branching and possible branches based off data state should be explicit,
  simple, and apparent at a glance.
- No single-use helper functions — inline them. Helpers exist when removing
  repetitive parts of code or if the utility is needed generally. In both
  cases, the utility functions should be used multiple times and not oneoffs.
- Public fields over getters; private is reserved for invariant-preserving
  state not a default. As a rule things should be public by default. An
  argument needs to be given for something to actually be private.
- Fixed-size arrays / const generics where counts are known (venue arity
  `[T; V]`, dispatch once at the top); no growable Vecs for known sizes.
  The aim should be to minimize allocations and resources. Preallocating and
  using fixed size buffers upfront should be working to keep things simpler.
- Reuse, don't recreate: list files once per process, reload buffers in
  place, pool builders, keep allocations out of hot paths. When adding a
  path, check what existing item already holds the data.
- Fail early: things that can fail give an error as soon as possible rather
  than keeping around invalid state or overriding invalid state with defaults.
  Failures at startup are better than runtime, failures on bad requests are
  easier to track down than odd behaviour in the core system. We can also then
  more easily handle errors when they occur rather than constantly handling
  invalid state.
- Use explicit types; avoid using strings and other structures where
  complexity may be hidden. DO NOT!!! just use box<str> or an equivalent the
  point is to enforce CORRECT REPRESENTATION instead of handwaving away
  complexity inherint to what you're doing. This works in conjunction with
  failing early. If it's not what you expect it should FAIL not get passed
  around as an unexpected/ invalid state of the program.
- `tracing`, not `println!` — except CLI output.
- Design should be data centric first, no grand sweeping OOP objects. Avoid
  OOP designs where you can, rust code is not just making rust structs with
  hundreds of impls. This code is disgusting and working with grand objects
  doesn't abstract well or work in larger systems. 
- At some point in the creation process you should reason from the main event
  loop through to where the changes/ additions are being done and make sure
  there isn't needless inderection, allocations, or work being done. Keep
  things simple relative to this context. You should be able to walkthrough
  from the main execution loop to all the systems being run and have an
  internal representation of the program. You need to use this understanding
  to keep the work on track, simple, and avoid duplication and unnecessary work.
  If you can't do that, this is an immediate problem.
- When making changes, especially for hot path, you should start first by doing
  a rough back of the envelope hand calculation for how fast you expect something
  to be and the performance concerns around what you're doing. You should then
  check the code against criterion where you can and for longer integration
  style benchmarks, you should be running against tracy. Verify your approach
  where you can taking performance considerations into account. Also check your
  work with cargo show asm for performance critical regions. When needed unsafe
  rust is allowed as long as you're doing the proper memory profiling and tests
  needed to verify that there isn't any issues. 

Repo Rules:
- git: Do not touch git without explicit instruction to from the user. Changes
  should be made and left in source control for review before ever committed.
- Cargo fmt: make sure you're running `cargo fmt --all` for your changes after you're
  done. This should happen AT LEAST before you're done with the changes you're
  actively working on and those changes are going to be reviewed in source control.
- `cargo check --lib --bins` with `RUSTFLAGS="-D warnings"` - No compiler warnings
- `cargo clippy --lib --bins -- -D warnings` - No clippy warnings
- Tests: Tests should be kept out of the main code files and follow a /tests convention.
  in tests/ there should be unit/ and integration/ and stuff like fixtures/ and the sort
  if needed. Integration tests should all be behind a feature flag and are used to verify
  the collective running logic. They should be the gold standard for actually verifying
  running collections of code.
- Dependencies: Ask before adding any to the Cargo.toml and when you do make sure to check
  what the latest versions of those are. There should be a cargo command for that. All
  dependencies should go in the workspace cargo toml and then shared out to the other
  crates from there. Try to keep the dependencies as minimal as we can instead of just
  adding them just bc it's easy. 
- Build Dependencies: You can use nix for these.
- If you need to make your own files or notes, you can use /tmp which is gitignored and
  will be cleaned up for you once it's no longer needed. So use it as you wish.
- When working on problems make sure before starting we work out all the work that needs
  to do be done, any additional information needed for it should be gathered if we can,
  a plan should be made ideally using plan mode that we can iterate on going back and
  forth with the user, and then look at if we can break up and organize this work for
  more agents to help out on. The splitting up of work should not sacrifice quality if
  some things are better handled together obviously, so some discerment is required there.

### Your own words on why you don't follow these instructions.
Why it keeps happening

I start from the call site I'm editing and ask "what happens here if X is missing?". That question can only be answered with control flow: a guard, an Option, an expect. The design question is "which type makes X impossible to build, and who owns it?", and I keep skipping to the first one.

Mass sed and perl rewrites make it worse. They keep the old control-flow shape and spread it across 60 sites, which is also where your review burden comes from.

Even my step-2 sketch was already control-flow shaped. set_focus -> Result hands the check to every caller instead of putting the operation with the owner.

Where it shows in steps 1–2

1. set_focus has 13 callers matching Ok/Err, or using .expect. The real gaps are two invariants:
   - The owner should do the operation. panes.rs asks Layout for a neighbour's id, then hands it back to set_focus with an expect. Layout should own "focus the pane to the left" and "focus the next pane" as operations.
   - A rendered view's Link::Follows { pane } can name a closed pane. Nothing unlinks it when the pane closes, so every use rechecks with contains_key or set_focus. The fix is that closing a pane turns its followers Standalone. A link then always names a live pane, and the checks disappear.
2. scroll.rs: set_focus(pane).ok()?. This turns an error back into a silent None, the exact thing this work is meant to remove.
3. Layout::from_parts checks a tree that session restore already checked, then gets .expect. check_node validates, then restore_node re-derives with expect("checked by check_node"). That's "check, then trust", not "parse into the type".
   - Session restore should parse SessionNode into Node in one pass that returns Result.
   - flex should deserialize straight into Flex through try_from, as Dragged does.
   - The layout should then be built from the parsed tree, so the panes and the tree can't disagree and there's nothing left to compare.
4. Bool-tuple matches:
   - from_parts matches on (leaves == keys, contains_key).
   - The session focus check is match focus < len { true => {}, false => return Err }. It should be session.panes.get(focus).ok_or(MissingFocus)?, where the check returns the pane as its evidence.

How I'll work from here

- Before writing anything for a change, answer the design questions in the plan: the states as enums, the owner of each invariant, and where bad input is rejected when values are built. The sketch I show you is that, not function signatures.
- No mass rewrites. If many call sites need the same change, that means the operation belongs with the owner. The fix is a method that makes those call sites shorter or unnecessary, not the same guard pasted 13 times.