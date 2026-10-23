# editor

A keyboard-driven editor with helix's keys built to replace my own existing workflows for coding,
obsidian, math, writing, and reading. Possibly missing core features used by others as this is
built for my own usage. 
For Linux, on Wayland or X11 with a Vulkan-capable GPU.

Disclaimer: built with the help of ai, incase it wasn't clear and that you care.


## Install

### With Nix

```sh
nix run <repo-url>                  # try it
nix profile install <repo-url>      # install `edt`
```

On a distribution other than NixOS, Nix programs may not find the system's GPU driver; run edt
through [nixGL](https://github.com/nix-community/nixGL) if the window fails to open.

### From source

You need Rust 1.98.1, the version `rust-toolchain.toml` pins ([rustup](https://rustup.rs),
a C and C++ compiler, `pkg-config`, and the development files for
libxcb, libxkbcommon, fontconfig, freetype and libgit2.
At run time edt also loads Wayland and Vulkan.

```sh
sudo apt install build-essential pkg-config libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libfontconfig-dev libfreetype-dev libgit2-dev libwayland-client0 libvulkan1
```

Then build and install the `edt` binary (the package is called `editor`):

```sh
cargo install --locked --git <repo-url> editor
```

## Use

```sh
edt               # the current folder
edt path/to/dir   # a folder
edt notes.md      # a file, in its git repository (or its folder when it is not in one)
```

## New file types

`*.pdt` - paged drawing canvas
`*.bdt` - board drawing canvas (general infinite whiteboard)

## Keybindings

[docs/keymap.md](docs/keymap.md) for keybinding list.
Keys are mostly helix with some custom ones.
Arrow keys are also aliased with hjkl.
- `space e` file tree
- `space g` diff tree
- `space ?` command pallet
Each file type has it's own collection of views.
So view switching here depends on the file type.
- `space d` switch to diff view
- `space v` split rendered view
- `spave V` toggle full rendered view
- `space tab` cycle views
tiling mostly comes down to `w` (window).
- `space w v` split
- `space w q` closes focused tile
- `space w h/l` switch between focused tile
- `space w </>` resize split tiles. (you can also use mouse)

os key + mouse you can move things around. 

For drawing:
- `u/U` for undo and redo respectively
- `e/b` erase and draw
- `A` adds a page for `*.pdt`
- `+/-/=` zoom and fit to page
- `hjkl` pan
- `]l/[l` next and previous layer
- `zh/zn` toggle active layer and add a new layer

## Configure

Read from `~/.config/edt/config.toml` (or the file given with `--config`).
All the settings are optional, closer to an override where it just follows
the default values otherwise. Fails on bad values or keys. You can see
`default-config.toml` which should show all the default values that you can
change. Should look like: 
```toml
[theme]
scheme = "default"          # built in; any other name reads schemes/<name>.toml beside this file
# background = "#004040"    # replaces base00 as the editor background

[theme.diff]                # each unset colour comes from the scheme
# added = "#rrggbb"
# removed = "#rrggbb"
# modified = "#rrggbb"
# ours = "#rrggbb"
# theirs = "#rrggbb"
# base = "#rrggbb"

[font.mono]                 # code
family = "Lilex"
size_px = 16.0

[font.ui]                   # interface; also prose unless [font.prose] is set
family = "IBM Plex Sans"
size_px = 14.0

# [font.prose]              # rendered markdown and typst text
# family = "Literata"
# size_px = 16.0

[session]
restore = "off"             # "last" reopens the tabs and splits from the last time in this folder

[tabs]
open_from_tree = "preview"  # a preview tab is replaced by the next preview opened in its pane
open_from_picker = "permanent"

[window]
title_bar = "hidden"        # "shown" draws a title bar
```

Font must be one of the bundled two or a font installed on the system.

### Colour schemes

A scheme is the 16 [base16](https://github.com/tinted-theming/home)
same sort of thing as stylix.
Put a scheme in `~/.config/edt/schemes/<name>.toml`
select it with `scheme = "<name>"`:

```toml
# ~/.config/edt/schemes/mine.toml
inherits = "default"   # optional: take every slot not listed here from another scheme
base00 = "#1d2021"
base08 = "#fb4934"
```

Without `inherits`, a scheme must set all 16 slots, `base00` to `base0F`. The keys are the ones
base16 schemes use, so an existing scheme can be copied in as it is.
See `default-scheme.toml` as reference.

## Develop

`nix develop` flake support for all extra libraries and what not.
The grammars compiled in are listed in `vendor/grammars/sources.nix`
after changing the list, run `vendor/grammars/update.sh`
this is the internal highlighting support for different file types *.rs etc.

## License

GPL-3.0-or-later. The vendored GPUI, helix and grammar sources keep their own licences, next to
their code.
