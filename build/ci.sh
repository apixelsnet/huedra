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

step "clippy (windows, sample data)"
# The test-only sample mode of the settings window (huedra-ui feature
# `sample`); release builds leave it out.
cargo xwin clippy -p huedra --features sample --target "$target" $locked -- -D warnings

step "deny"
cargo deny --workspace check

step "third-party licenses"
# Rewritten in the source tree, like Cargo.lock: remote-build.sh copies it
# back, and CI fails when the committed copy differs.
bash build/licenses.sh

step "test"
cargo test --workspace $locked

step "extension icons"
icons=$(mktemp -d)
cargo run -q -p huedra-core --example extension_icons $locked -- "$icons"
for icon in "$icons"/*.png; do
  if ! cmp -s "$icon" "extras/browser/icons/${icon##*/}"; then
    # Kept for the CI run's artifacts, to be committed.
    mkdir -p "${CARGO_TARGET_DIR:-target}/extension-icons"
    cp "$icons"/*.png "${CARGO_TARGET_DIR:-target}/extension-icons/"
    echo "extras/browser/icons is out of date: run the extension_icons example (CI keeps the new icons as the artifact extension-icons)" >&2
    exit 1
  fi
done

step "doc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps $locked

step "build (windows release)"
cargo xwin build --release --target "$target" $locked
exe="${CARGO_TARGET_DIR:-target}/$target/release/huedra.exe"
ls -l "$exe"

step "size (at most 12 MB)"
size=$(stat -c %s "$exe")
echo "$size of 12000000 bytes"
[ "$size" -le 12000000 ] || { echo "huedra.exe is larger than 12 MB" >&2; exit 1; }
