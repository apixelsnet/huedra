#!/usr/bin/env bash
# Formats the sources with rustfmt on the build server and copies the
# formatted crates back. Rust is not needed on this machine.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if [ -f build/local.env ]; then . build/local.env; fi
: "${HUEDRA_BUILD_HOST:?set HUEDRA_BUILD_HOST or create build/local.env}"
host=$HUEDRA_BUILD_HOST
remote=build/huedra

echo "==> sync source to $host"
tar --exclude=./target --exclude=./dist --exclude=./.git --exclude=./.claude \
    --exclude=./CLAUDE.md --exclude=./AGENTS.md --exclude=./build/local.env -czf - . \
  | ssh "$host" "rm -rf $remote/src && mkdir -p $remote/src && tar -xzf - -C $remote/src"

echo "==> cargo fmt"
ssh "$host" "docker run --rm -v \$HOME/$remote/src:/src -w /src huedra-build cargo fmt --all"

echo "==> copy formatted crates back"
ssh "$host" "tar -czf - -C $remote/src crates" | tar -xzf - -C "$root"
git -C "$root" status --short crates
