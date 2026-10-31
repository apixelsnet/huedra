# Build image for Huedra: the pinned Rust toolchain plus cargo-xwin for the
# Windows MSVC target and cargo-deny for dependency checks.
FROM rust:1.99.1-bookworm

RUN apt-get update \
 && apt-get install -y --no-install-recommends clang lld llvm \
 && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy \
 && rustup target add x86_64-pc-windows-msvc

ARG CARGO_XWIN_VERSION=0.23.1
ARG CARGO_DENY_VERSION=0.20.2
RUN cargo install --locked "cargo-xwin@${CARGO_XWIN_VERSION}" "cargo-deny@${CARGO_DENY_VERSION}" \
 && rm -rf /usr/local/cargo/registry

# cargo-xwin downloads Microsoft's C runtime and Windows SDK files. Accepting
# Microsoft's free terms for those files is a recorded project decision
# (docs/features/12-packaging.md). No paid licence is involved.
ENV XWIN_ACCEPT_LICENSE=1 \
    XWIN_CACHE_DIR=/xwin-cache
