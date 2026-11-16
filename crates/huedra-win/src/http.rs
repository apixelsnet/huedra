//! HTTPS requests through WinHTTP, which uses the encryption built into
//! Windows, so no TLS library is included. Used by the settings process only.
//! Feature doc: `docs/features/13-game-info.md`.

use std::ffi::c_void;
use std::ptr;

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_URL, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle,
    WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpQueryOption,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::core::{HSTRING, PCWSTR, w};

const RESOLVE_MS: i32 = 5_000;
const CONNECT_MS: i32 = 10_000;
const TRANSFER_MS: i32 = 15_000;

/// A completed request.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP status code.
    pub status: u32,
    /// Body.
    pub body: Vec<u8>,
    /// The address after redirects.
    pub final_url: String,
}

/// A request failed.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// WinHTTP reported an error (no network, timeout, certificate problem).
    #[error("the request failed")]
    Windows(#[from] windows::core::Error),
    /// The response was larger than allowed.
    #[error("the response is larger than {0} bytes")]
    TooLarge(usize),
    /// The download could not be written to its file.
    #[error("the download could not be saved: {0}")]
    File(#[from] std::io::Error),
}

struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a WinHTTP handle opened in this module, closed once.
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn check(handle: *mut c_void) -> Result<Handle, HttpError> {
    if handle.is_null() {
        Err(windows::core::Error::from_thread().into())
    } else {
        Ok(Handle(handle))
    }
}

/// `GET https://{host}{path}`, following redirects, reading at most
/// `max_bytes` of body.
///
/// # Errors
/// [`HttpError`] on network failures and oversized responses.
pub fn get(
    host: &str,
    path: &str,
    user_agent: &str,
    max_bytes: usize,
) -> Result<Response, HttpError> {
    let mut body = Vec::new();
    let (status, final_url) = request(host, path, user_agent, max_bytes, &mut |chunk| {
        body.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(Response {
        status,
        body,
        final_url,
    })
}

/// `GET https://{host}{path}` into a file, following redirects, reading at
/// most `max_bytes`, without holding the body in memory. Returns the HTTP
/// status; the file holds whatever body came with it.
///
/// # Errors
/// [`HttpError`] on network failures, oversized responses and file errors.
pub fn download(
    host: &str,
    path: &str,
    user_agent: &str,
    max_bytes: usize,
    file: &std::path::Path,
) -> Result<u32, HttpError> {
    use std::io::Write as _;
    let mut out = std::io::BufWriter::new(std::fs::File::create(file)?);
    let (status, _) = request(host, path, user_agent, max_bytes, &mut |chunk| {
        out.write_all(chunk)
    })?;
    out.flush()?;
    Ok(status)
}

/// One request; each piece of the body goes to `sink`. Returns the status
/// and the address after redirects.
fn request(
    host: &str,
    path: &str,
    user_agent: &str,
    max_bytes: usize,
    sink: &mut dyn FnMut(&[u8]) -> std::io::Result<()>,
) -> Result<(u32, String), HttpError> {
    // SAFETY: all strings outlive the calls; handles are closed by guards in
    // reverse order of opening.
    let session = check(unsafe {
        WinHttpOpen(
            &HSTRING::from(user_agent),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    })?;
    // SAFETY: a valid session handle.
    unsafe { WinHttpSetTimeouts(session.0, RESOLVE_MS, CONNECT_MS, TRANSFER_MS, TRANSFER_MS) }?;
    // SAFETY: as above.
    let connection = check(unsafe {
        WinHttpConnect(
            session.0,
            &HSTRING::from(host),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        )
    })?;
    // SAFETY: as above; a null accept-types list means any type.
    let request = check(unsafe {
        WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            &HSTRING::from(path),
            PCWSTR::null(),
            PCWSTR::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    })?;
    // SAFETY: sends the request with no extra headers or body.
    unsafe { WinHttpSendRequest(request.0, None, None, 0, 0, 0) }?;
    // SAFETY: waits for the response headers.
    unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) }?;

    let mut status = 0_u32;
    let mut size = 4_u32;
    let mut index = 0_u32;
    // SAFETY: `status` is a 4-byte buffer for the numeric status code.
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&raw mut status).cast()),
            &mut size,
            &mut index,
        )
    }?;

    let mut total = 0_usize;
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        let mut read = 0_u32;
        // SAFETY: `chunk` holds the requested number of bytes.
        unsafe {
            WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                u32::try_from(chunk.len()).unwrap_or(0),
                &mut read,
            )
        }?;
        if read == 0 {
            break;
        }
        total += read as usize;
        if total > max_bytes {
            return Err(HttpError::TooLarge(max_bytes));
        }
        sink(&chunk[..read as usize])?;
    }

    Ok((status, final_url(&request).unwrap_or_default()))
}

fn final_url(request: &Handle) -> Option<String> {
    let mut buffer = vec![0_u16; 2048];
    let mut bytes = u32::try_from(buffer.len() * 2).ok()?;
    // SAFETY: `buffer` holds `bytes` bytes for the URL.
    unsafe {
        WinHttpQueryOption(
            request.0,
            WINHTTP_OPTION_URL,
            Some(buffer.as_mut_ptr().cast()),
            &mut bytes,
        )
    }
    .ok()?;
    let len = (bytes as usize / 2).min(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..len]);
    Some(text.trim_end_matches('\0').to_owned())
}
