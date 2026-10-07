{
  description = "Editor development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
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
              pkgs.helix.runtime
            ];

            buildInputs = runtimeLibs ++ (with pkgs; [
              zstd
              openssl
              libgit2
              alsa-lib
            ]);

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
            HELIX_RUNTIME = "${pkgs.helix.runtime}";
          };
        });
    };
}
