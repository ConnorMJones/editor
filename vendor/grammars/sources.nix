# Lists the grammars compiled into the binary, at the revisions the pinned helix names.
# Run `vendor/grammars/update.sh` after changing `wanted`.
let
  helix = (builtins.getFlake (toString ../..)).inputs.helix-src;
  config = builtins.fromTOML (builtins.readFile "${helix}/languages.toml");
  wanted = [
    "rust" "rust-format-args" "c" "cpp" "python" "javascript" "jsdoc" "typescript" "tsx"
    "go" "go-format-string" "lua" "lua-format-string" "luap" "zig" "bash" "nix" "toml" "yaml"
    "json" "markdown" "markdown_inline" "typst" "latex" "html" "css" "make" "dockerfile"
    "gitcommit" "git-rebase" "gitignore" "git-config" "diff" "comment" "regex"
  ];
  grammars = builtins.filter (g: builtins.elem g.name wanted) config.grammar;
  fetch = g:
    let github = builtins.match "https://github\\.com/([^/]*)/([^/]*)/?" g.source.git;
    in if github != null
      then builtins.fetchTree {
        type = "github";
        owner = builtins.elemAt github 0;
        repo = builtins.elemAt github 1;
        inherit (g.source) rev;
      }
      else builtins.fetchTree {
        type = "git";
        url = g.source.git;
        inherit (g.source) rev;
        ref = g.source.ref or "HEAD";
        shallow = true;
      };
in {
  helix = "${helix}";
  grammars = map (g: {
    inherit (g) name;
    inherit (g.source) git rev;
    repo = (fetch g).outPath;
    subpath = g.source.subpath or "";
  }) grammars;
  languages = map (l: l.name)
    (builtins.filter (l: builtins.elem (l.grammar or l.name) wanted) config.language);
}
