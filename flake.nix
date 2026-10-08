{
  description = "Editor development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Must match the vendored helix-core revision so runtime queries compile.
    helix-src = {
      url = "github:helix-editor/helix/ba40e547426b0f9896c8bdc699a4ab11f2b37dbc";
      flake = false;
    };
  };

  outputs = { nixpkgs, rust-overlay, helix-src, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          # perl fails to compile under GCC 15.
          helixGrammars = pkgs.callPackage "${helix-src}/grammars.nix" {
            includeGrammarIf = grammar: grammar.name != "perl";
          };
          helixRuntime = pkgs.runCommand "helix-runtime" { } ''
            mkdir -p $out
            ln -s ${helix-src}/runtime/* $out
            rm -r $out/grammars
            ln -s ${helixGrammars} $out/grammars
          '';

          # Loaded with dlopen at runtime by gpui (wgpu, wayland, x11).
          runtimeLibs = with pkgs; [
            wayland
            libxkbcommon
            vulkan-loader
            libglvnd
            fontconfig
            freetype
            libx11
            libxcursor
            libxi
            libxrandr
            libxcb
          ];
        in
        {
          default = pkgs.mkShell {
            packages = [
              toolchain
              pkgs.pkg-config
              pkgs.cmake
              pkgs.clang
              pkgs.mold
              pkgs.perl
              pkgs.cargo-criterion
              pkgs.cargo-show-asm
              helixRuntime
            ];

            buildInputs = runtimeLibs ++ (with pkgs; [
              zstd
              openssl
              libgit2
              alsa-lib
            ]);

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
            HELIX_RUNTIME = "${helixRuntime}";
          };
        });
    };
}
