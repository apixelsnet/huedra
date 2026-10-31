#!/usr/bin/env bash
# Runs inside the huedra-build container: format, lint, dependency checks,
# tests and docs on Linux, then the Windows release build.
set -euo pipefail

target=x86_64-pc-windows-msvc
locked=--locked
[ -f Cargo.lock ] || locked=   # the first build creates the lock file

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

step "doc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps $locked

step "build (windows release)"
cargo xwin build --release --target "$target" $locked
ls -l "${CARGO_TARGET_DIR:-target}/$target/release/huedra.exe"
