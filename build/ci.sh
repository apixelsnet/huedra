#!/usr/bin/env bash
# Runs inside the huedra-build container: format, lint, dependency checks,
# tests and docs on Linux, then the Windows release build.
set -euo pipefail

target=x86_64-pc-windows-msvc
locked=--locked

# Record new or changed dependencies in Cargo.lock (minimal update), then
# check everything against that lock. remote-build.sh copies it back.
cargo fetch -q

step() { printf '\n==> %s\n' "$*"; }

step "fmt"
cargo fmt --all --check

step "clippy (linux)"
cargo clippy --workspace --all-targets $locked -- -D warnings

step "clippy (windows)"
cargo xwin clippy --workspace --all-targets --target "$target" $locked -- -D warnings

step "deny"
cargo deny --workspace check

step "test"
cargo test --workspace $locked

step "extension icons"
icons=$(mktemp -d)
cargo run -q -p huedra-core --example extension_icons $locked -- "$icons"
for icon in "$icons"/*.png; do
  cmp -s "$icon" "extras/browser/icons/${icon##*/}" || { echo "extras/browser/icons is out of date: run the extension_icons example" >&2; exit 1; }
done

step "doc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps $locked

step "build (windows release)"
cargo xwin build --release --target "$target" $locked
exe="${CARGO_TARGET_DIR:-target}/$target/release/huedra.exe"
ls -l "$exe"

step "size (at most 10 MB)"
size=$(stat -c %s "$exe")
echo "$size of 10000000 bytes"
[ "$size" -le 10000000 ] || { echo "huedra.exe is larger than 10 MB" >&2; exit 1; }
