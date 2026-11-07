#!/usr/bin/env bash
# Builds Huedra on the build server and copies huedra.exe to dist/.
#
# The server is read from HUEDRA_BUILD_HOST, or from build/local.env
# (git-ignored), e.g.:  HUEDRA_BUILD_HOST=my-server
#
# With --sample, it also builds dist/huedra-sample.exe: the settings window
# with sample games, displays and simulated HDR (feature `sample`), for
# screenshots and tests. Never publish that build.
#
# HUEDRA_UPDATE_KEY (minisign public key, base64 line) and HUEDRA_UPDATE_URL
# (https release folder ending in /) are passed to the build when set;
# without both, the program does not look for updates
# (docs/features/11-updates.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if [ -f build/local.env ]; then . build/local.env; fi
: "${HUEDRA_BUILD_HOST:?set HUEDRA_BUILD_HOST or create build/local.env}"
host=$HUEDRA_BUILD_HOST
remote=build/huedra   # relative to the remote home directory

echo "==> sync source to $host"
tar --exclude=./target --exclude=./dist --exclude=./.git --exclude=./.claude \
    --exclude=./CLAUDE.md --exclude=./AGENTS.md --exclude=./build/local.env -czf - . \
  | ssh "$host" "rm -rf $remote/src && mkdir -p $remote/src $remote/target && tar -xzf - -C $remote/src"

echo "==> build image"
ssh "$host" "docker build -q -t huedra-build $remote/src/build >/dev/null"

update_env=""
for name in HUEDRA_UPDATE_KEY HUEDRA_UPDATE_URL; do
  if [ -n "${!name:-}" ]; then update_env+=" -e $name=$(printf %q "${!name}")"; fi
done

echo "==> check, test, build"
ssh "$host" "docker run --rm$update_env \
  -v \$HOME/$remote/src:/src -v \$HOME/$remote/target:/target -e CARGO_TARGET_DIR=/target \
  -v huedra-cargo:/usr/local/cargo/registry -v huedra-advisory:/usr/local/cargo/advisory-dbs \
  -v huedra-xwin:/xwin-cache -w /src huedra-build bash build/ci.sh"

if [ "${1:-}" = "--sample" ]; then
  echo "==> build with sample data"
  ssh "$host" "docker run --rm$update_env \
    -v \$HOME/$remote/src:/src -v \$HOME/$remote/target:/target -e CARGO_TARGET_DIR=/target \
    -v huedra-cargo:/usr/local/cargo/registry -v huedra-xwin:/xwin-cache -w /src huedra-build \
    bash -c 'cargo xwin build --release --target x86_64-pc-windows-msvc --locked -p huedra --features sample \
      && cp /target/x86_64-pc-windows-msvc/release/huedra.exe /target/huedra-sample.exe \
      && cargo xwin build --release --target x86_64-pc-windows-msvc --locked'"
fi

echo "==> fetch"
mkdir -p dist
scp -q "$host:$remote/target/x86_64-pc-windows-msvc/release/huedra.exe" dist/huedra.exe
if [ "${1:-}" = "--sample" ]; then
  scp -q "$host:$remote/target/huedra-sample.exe" dist/huedra-sample.exe
fi
scp -q "$host:$remote/src/Cargo.lock" Cargo.lock
scp -q "$host:$remote/src/THIRD-PARTY-LICENSES.md" THIRD-PARTY-LICENSES.md
ls -l dist/huedra.exe
