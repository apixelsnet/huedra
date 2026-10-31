//! Every call Huedra makes into Windows, and all of its `unsafe` code.
//!
//! Each `unsafe` block carries a `// SAFETY:` comment. The other crates
//! forbid `unsafe` entirely. Filled in from phase 1, starting with
//! `docs/features/01-hdr-switch.md`.
