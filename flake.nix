{
  description = "edt: a helix-style editor with source review, markdown and typst";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # The vendored helix-core revision; vendor/grammars/sources.nix reads its grammar pins and queries.
    helix-src = {
      url = "github:helix-editor/helix/ba40e547426b0f9896c8bdc699a4ab11f2b37dbc";
      flake = false;
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem = f: nixpkgs.lib.genAttrs systems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
        in
        f {
          inherit pkgs;
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
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
          buildLibs = with pkgs; [
            zstd
            openssl
            libgit2
            alsa-lib
          ];
        });
    in
    {
      packages = perSystem ({ pkgs, toolchain, runtimeLibs, buildLibs }:
        let
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          edt = rustPlatform.buildRustPackage {
            pname = "edt";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
            src = pkgs.lib.cleanSource ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };
            cargoBuildFlags = [ "-p" "editor" ];
            # The suites need a display and the `integration` feature; run them from the dev shell.
            doCheck = false;
            nativeBuildInputs = with pkgs; [ pkg-config cmake clang makeWrapper ];
            buildInputs = runtimeLibs ++ buildLibs;
            postFixup = ''
              wrapProgram $out/bin/edt --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
            '';
            meta = {
              description = "A helix-style editor with source review, markdown and typst";
              mainProgram = "edt";
              platforms = systems;
            };
          };
        in
        {
          inherit edt;
          default = edt;
        });

      devShells = perSystem ({ pkgs, toolchain, runtimeLibs, buildLibs }: {
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
          ];

          buildInputs = runtimeLibs ++ buildLibs;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
        };
      });
    };
}
