#!/usr/bin/env bash
# Copies the grammars and queries that sources.nix lists into this folder.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
eval_raw() { nix eval --impure --raw -f "$here/sources.nix" --apply "$1"; }

helix=$(eval_raw 's: s.helix')
grammars=$(eval_raw 's: builtins.concatStringsSep "\n" (map (g: "${g.name} ${g.repo} ${if g.subpath == "" then "." else g.subpath} ${g.git} ${g.rev}") s.grammars)')
languages=$(eval_raw 's: builtins.concatStringsSep " " s.languages')

chmod -R u+w "$here/parsers" "$here/queries" 2>/dev/null || true
rm -rf "$here/parsers" "$here/queries"
printf '# name git rev\n' > "$here/SOURCES"
while read -r name repo subpath git rev; do
  dir="$here/parsers/$name"
  mkdir -p "$dir"
  cp -r "$repo/$subpath/src/." "$dir"
  chmod -R u+w "$dir"
  rm -f "$dir/grammar.json" "$dir/node-types.json"
  # Grammars in a subfolder can share their repository's `common` folder.
  if [ "$subpath" != . ] && [ -d "$repo/common" ]; then
    cp -r "$repo/common" "$dir/common"
    chmod -R u+w "$dir/common"
    sed -i 's#"../../common/#"common/#' "$dir"/scanner.*
  fi
  license=$(ls "$repo/$subpath"/LICEN[CS]E* "$repo"/LICEN[CS]E* "$repo"/COPYING* 2>/dev/null | head -1 || true)
  cp "$license" "$dir/LICENSE"
  printf '%s %s %s\n' "$name" "$git" "$rev" >> "$here/SOURCES"
done <<< "$grammars"

# Queries for every language on these grammars, plus the folders they inherit from.
pending=$languages
while [ -n "$pending" ]; do
  next=""
  for lang in $pending; do
    [ -d "$here/queries/$lang" ] && continue
    [ -d "$helix/runtime/queries/$lang" ] || continue
    mkdir -p "$here/queries/$lang"
    cp "$helix/runtime/queries/$lang/"*.scm "$here/queries/$lang/"
    chmod u+w "$here/queries/$lang/"*.scm
    next="$next $(sed -n 's/^; inherits: *//p' "$here/queries/$lang/"*.scm | tr ',' ' ')"
  done
  pending=$(echo $next)
done
