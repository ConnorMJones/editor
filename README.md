# editor

A keyboard-driven editor with helix's keys built to replace my own existing workflows for coding,
obsidian, math, writing, and reading. Possibly missing core features used by others as this is
built for my own usage. 
For Linux, on Wayland or X11 with a Vulkan-capable GPU.

Disclaimer: built with the help of ai, incase it wasn't clear and that you care.


## Install

Everything edt needs at run time is inside the binary: the syntax grammars and their queries, the
default fonts (Lilex and IBM Plex Sans) and the default colour scheme. Nothing is downloaded.

### With Nix

```sh
nix run <repo-url>                  # try it
nix profile install <repo-url>      # install `edt`
```

On a distribution other than NixOS, Nix programs may not find the system's GPU driver; run edt
through [nixGL](https://github.com/nix-community/nixGL) if the window fails to open.

### From source

You need Rust 1.98.1, the version `rust-toolchain.toml` pins ([rustup](https://rustup.rs) installs
it), a C and C++ compiler, `pkg-config`, and the development files for libxcb, libxkbcommon, fontconfig, freetype and libgit2. At run time edt
also loads Wayland and Vulkan. On Debian or Ubuntu that is:

```sh
sudo apt install build-essential pkg-config libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libfontconfig-dev libfreetype-dev libgit2-dev libwayland-client0 libvulkan1
```

Then build and install the `edt` binary (the package is called `editor`):

```sh
cargo install --locked --git <repo-url> editor
```

A release build takes several minutes.

## Use

```sh
edt               # the current folder
edt path/to/dir   # a folder
edt notes.md      # a file, in its git repository (or its folder when it is not in one)
```

The keys are helix's, plus a few of edt's own: `space e` file tree, `space g` Changes, `space d`
diff, `space v` rendered split, `space ?` command palette. [docs/keymap.md](docs/keymap.md) lists
every key.

## Configure

edt reads `~/.config/edt/config.toml` (or the file given with `--config`). Every setting is
optional; an unknown key or a bad value stops edt at startup with the reason. The defaults:

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

A font family must be one of the bundled two or a font installed on the system.

### Colour schemes

A scheme is the 16 [base16](https://github.com/tinted-theming/home) colours; edt maps them onto
the interface, syntax and diff colours. Put a scheme in `~/.config/edt/schemes/<name>.toml` and
select it with `scheme = "<name>"`:

```toml
# ~/.config/edt/schemes/mine.toml
inherits = "default"   # optional: take every slot not listed here from another scheme
base00 = "#1d2021"
base08 = "#fb4934"
```

Without `inherits`, a scheme must set all 16 slots, `base00` to `base0F`. The keys are the ones
base16 schemes use, so an existing scheme can be copied in as it is.

### Stylix

[Stylix](https://github.com/nix-community/stylix) has no target for edt yet. With home-manager,
write its colours as a scheme:

```nix
xdg.configFile."edt/schemes/stylix.toml".text =
  with config.lib.stylix.colors.withHashtag; ''
    base00 = "${base00}"
    base01 = "${base01}"
    base02 = "${base02}"
    base03 = "${base03}"
    base04 = "${base04}"
    base05 = "${base05}"
    base06 = "${base06}"
    base07 = "${base07}"
    base08 = "${base08}"
    base09 = "${base09}"
    base0A = "${base0A}"
    base0B = "${base0B}"
    base0C = "${base0C}"
    base0D = "${base0D}"
    base0E = "${base0E}"
    base0F = "${base0F}"
  '';
```

and set `scheme = "stylix"` in `config.toml`.

## Develop

`nix develop` gives the toolchain and libraries. The checks run before every change are
`cargo fmt --all`, `RUSTFLAGS="-D warnings" cargo check --lib --bins`,
`cargo clippy --lib --bins -- -D warnings` and `cargo test -p state --features integration`.
`docs/design.md` describes how the code is written.

The grammars compiled in are listed in `vendor/grammars/sources.nix`; after changing the list, run
`vendor/grammars/update.sh` to copy their sources and queries into the repository.

## License

GPL-3.0-or-later. The vendored GPUI, helix and grammar sources keep their own licences, next to
their code.
