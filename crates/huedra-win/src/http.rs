//! HTTPS requests through WinHTTP, which uses the encryption built into
//! Windows, so no TLS library is included. Used by the settings process only.
//! Feature doc: `docs/features/13-game-info.md`.

use std::ffi::c_void;
use std::ptr;

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_URL, WINHTTP_QUERY_CONTENT_LENGTH, WINHTTP_QUERY_CONTENT_RANGE,
    WINHTTP_QUERY_ETAG, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LAST_MODIFIED,
    WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpQueryOption, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
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

/// What a server says about a file: enough to tell a changed file from the
/// same one without downloading it again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    /// `ETag`, as sent; empty when not sent.
    pub etag: String,
    /// `Last-Modified`, as sent; empty when not sent.
    pub modified: String,
    /// The whole file's size in bytes, when the server says.
    pub length: Option<u64>,
    /// The address the file came from, after redirects; empty when Windows
    /// doesn't say.
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
    let (status, final_url, _) = request(host, path, user_agent, None, &mut |chunk| {
        body.extend_from_slice(chunk);
        if body.len() > max_bytes {
            return Err(std::io::Error::other(TOO_LARGE));
        }
        Ok(())
    })
    .map_err(|e| too_large(e, max_bytes))?;
    Ok(Response {
        status,
        body,
        final_url,
    })
}

/// `GET https://{host}{path}` into a file, following redirects, reading at
/// most `max_bytes`, without holding the body in memory. Returns the HTTP
/// status and what the server said about the file; the file holds whatever
/// body came with it.
///
/// # Errors
/// [`HttpError`] on network failures, oversized responses and file errors.
pub fn download(
    host: &str,
    path: &str,
    user_agent: &str,
    max_bytes: usize,
    file: &std::path::Path,
) -> Result<(u32, Headers), HttpError> {
    use std::io::Write as _;
    let mut out = std::io::BufWriter::new(std::fs::File::create(file)?);
    let mut total = 0_usize;
    let (status, _, headers) = request(host, path, user_agent, None, &mut |chunk| {
        total += chunk.len();
        if total > max_bytes {
            return Err(std::io::Error::other(TOO_LARGE));
        }
        out.write_all(chunk)
    })
    .map_err(|e| too_large(e, max_bytes))?;
    out.flush()?;
    Ok((status, headers))
}

/// Asks for the first byte of `https://{host}{path}` only, following
/// redirects, and reads what the server says about the whole file. The body
/// is never read, so a server that ignores the range costs nothing more.
/// A `GET`, not a `HEAD`: download links that redirect to signed addresses
/// are often signed for `GET` only.
///
/// # Errors
/// [`HttpError`] on network failures.
pub fn probe(host: &str, path: &str, user_agent: &str) -> Result<(u32, Headers), HttpError> {
    let (status, _, headers) = request(
        host,
        path,
        user_agent,
        Some("Range: bytes=0-0"),
        &mut |_| Ok(()),
    )?;
    Ok((status, headers))
}

/// The message of the error a sink returns when the body is too large.
const TOO_LARGE: &str = "huedra-too-large";

/// Turns the sink's size error back into [`HttpError::TooLarge`].
fn too_large(error: HttpError, max_bytes: usize) -> HttpError {
    match error {
        HttpError::File(e) if e.to_string() == TOO_LARGE => HttpError::TooLarge(max_bytes),
        other => other,
    }
}

/// One request; each piece of the body goes to `sink`, or none when
/// `extra` asks for a range (see [`probe`]). Returns the status, the address
/// after redirects and what the server said about the file.
fn request(
    host: &str,
    path: &str,
    user_agent: &str,
    extra: Option<&str>,
    sink: &mut dyn FnMut(&[u8]) -> std::io::Result<()>,
) -> Result<(u32, String, Headers), HttpError> {
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
    let extra_wide: Option<Vec<u16>> = extra.map(|h| h.encode_utf16().collect());
    // SAFETY: sends the request with the extra headers, if any (which outlive
    // the call), and no body.
    unsafe { WinHttpSendRequest(request.0, extra_wide.as_deref(), None, 0, 0, 0) }?;
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

    let headers = Headers {
        etag: header(&request, WINHTTP_QUERY_ETAG),
        modified: header(&request, WINHTTP_QUERY_LAST_MODIFIED),
        length: whole_length(
            status,
            &header(&request, WINHTTP_QUERY_CONTENT_RANGE),
            &header(&request, WINHTTP_QUERY_CONTENT_LENGTH),
        ),
        final_url: String::new(),
    };
    let mut headers = headers;
    headers.final_url = final_url(&request).unwrap_or_default();
    if extra.is_some() {
        return Ok((status, headers.final_url.clone(), headers));
    }

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
        sink(&chunk[..read as usize])?;
    }

    Ok((status, headers.final_url.clone(), headers))
}

/// A response header as text; empty when not sent.
fn header(request: &Handle, which: u32) -> String {
    let mut buffer = [0_u16; 512];
    let mut bytes = u32::try_from(size_of_val(&buffer)).unwrap_or(0);
    let mut index = 0_u32;
    // SAFETY: `buffer` holds `bytes` bytes; Windows writes at most that many.
    let read = unsafe {
        WinHttpQueryHeaders(
            request.0,
            which,
            PCWSTR::null(),
            Some(buffer.as_mut_ptr().cast()),
            &mut bytes,
            &mut index,
        )
    };
    if read.is_err() {
        return String::new();
    }
    let len = (bytes as usize / 2).min(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

/// The whole file's size: from `Content-Range` (`bytes 0-0/12345`) on a
/// partial answer, else from `Content-Length` on a whole one.
fn whole_length(status: u32, range: &str, length: &str) -> Option<u64> {
    if status == 206 {
        return range.rsplit('/').next()?.trim().parse().ok();
    }
    if status == 200 {
        return length.trim().parse().ok();
    }
    None
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
