#!/usr/bin/env bash
# Writes THIRD-PARTY-LICENSES.md: the licences of every library built into
# huedra.exe. Run by build/ci.sh in the huedra-build container, after
# `cargo fetch`. Feature doc: docs/features/12-packaging.md.
set -euo pipefail

target=x86_64-pc-windows-msvc
out=THIRD-PARTY-LICENSES.md

# Licence texts come from the crates themselves, never from a web service,
# so the file is the same on every machine.
cargo about generate --locked --offline -c build/about.toml build/about.hbs -o "$out"

# Slint is used under its royalty-free licence (LicenseRef-Slint-Royalty-free-2.0).
# cargo-about 0.9 drops the text of LicenseRef licences even when it is
# clarified, so it is added here from Slint's own crate. The digest makes the
# build fail when Slint changes the text, so a new text is read before it ships.
slint_text_sha256=5167f5056e850419106ab6265efbdca7cba4d99c849d1445ca0bbf6a1e2315fe

crates=$(cargo tree --locked --offline -p huedra --target "$target" -e normal \
  --prefix none --format '{p}' | sed 's/ (.*//' | grep -E '^(i-)?slint(-[a-z-]+)? v' | sort -u)
version=$(sed -n 's/^slint v//p' <<<"$crates")
[ -n "$version" ] || { echo "licences: slint is not in the dependency tree" >&2; exit 1; }

text=$(echo "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/"slint-$version"/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)
[ -f "$text" ] || { echo "licences: $text not found" >&2; exit 1; }
if [ "$(sha256sum "$text" | cut -d' ' -f1)" != "$slint_text_sha256" ]; then
  echo "licences: Slint's royalty-free licence text changed; read it, then update slint_text_sha256 in build/licences.sh" >&2
  exit 1
fi

{
  echo "## Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0"
  echo
  echo "Used by:"
  while read -r name ver; do
    echo "- [$name ${ver#v}](https://crates.io/crates/$name)"
  done <<<"$crates"
  echo
  echo '````text'
  cat "$text"
  echo '````'
} >> "$out"

# Some crates ship their licence with CRLF line endings; the file uses LF.
sed -i 's/\r$//' "$out"
