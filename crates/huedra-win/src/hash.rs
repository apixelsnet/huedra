//! SHA-256 through Windows' own cryptography (CNG), so no hashing library
//! is included. Feature doc: `docs/features/11-updates.md`.

use windows::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};

/// The SHA-256 digest of `data`.
///
/// # Errors
/// The Windows error when CNG refuses the request.
pub fn sha256(data: &[u8]) -> windows::core::Result<[u8; 32]> {
    let mut digest = [0_u8; 32];
    // SAFETY: the SHA-256 pseudo-handle needs no opening or closing; `data`
    // and `digest` are valid slices and the digest is exactly 32 bytes.
    unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut digest) }.ok()?;
    Ok(digest)
}

/// The SHA-256 digest of `data` in lowercase hex; empty when Windows
/// refuses the request, so it never matches a recorded digest.
pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data)
        .map(|digest| digest.iter().map(|b| format!("{b:02x}")).collect())
        .unwrap_or_default()
}
