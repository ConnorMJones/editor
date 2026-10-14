#!/bin/sh
# Builds a throwaway git repo with every kind of change the editor's git views show.
set -eu
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

repo=$(mktemp -d "${TMPDIR:-/tmp}/editor-git-example.XXXXXX")
cd "$repo"

git init -q -b main
git config user.name "Example"
git config user.email "example@example.invalid"
git config commit.gpgsign false

cat > notes.md <<'MD'
# Notes

First line.
Second line.
Third line.
MD
printf 'fn main() {\n    println!("hello");\n}\n' > main.rs
printf 'old file\n' > obsolete.txt
printf 'staged\n' > staged.txt
git add .
git commit -q -m "Initial commit"

git checkout -q -b feature
sed 's/Second line./Second line, from the feature branch./' notes.md > notes.tmp
mv notes.tmp notes.md
git commit -q -am "Edit notes on feature"

git checkout -q main
sed 's/Second line./Second line, from main./' notes.md > notes.tmp
mv notes.tmp notes.md
git commit -q -am "Edit notes on main"

git merge -q feature >/dev/null 2>&1 || true

printf 'staged\nthis line is staged\n' > staged.txt
git add staged.txt
printf 'staged\nthis line is staged\nthis line is not\n' > staged.txt
printf 'fn main() {\n    println!("hello, world");\n}\n' > main.rs
rm obsolete.txt
printf 'not tracked yet\n' > untracked.txt

echo "$repo"
