//! The per-user named pipe between the tray process and its clients.
//!
//! The pipe is named after the user's SID, admits only that SID (DACL),
//! rejects remote clients, and is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`
//! so a second tray process notices the first. Clients check that the server
//! runs as the same user and connect at identification level only.
//! Feature doc: `docs/features/09-cli-ipc.md`.

use std::mem::size_of;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use huedra_core::ipc::{self, IpcError, MAX_MESSAGE_BYTES, Request, Response};
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GENERIC_READ,
    GENERIC_WRITE, HANDLE,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    EqualSid, GetTokenInformation, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_NONE, FlushFileBuffers, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX, ReadFile, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::{HSTRING, PWSTR};

use crate::event_loop::Waker;
use crate::handle::{LocalMemory, OwnedHandle, SendHandle};

const BUFFER_BYTES: u32 = 64 * 1024;
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
const BUSY_WAIT_MS: u32 = 2_000;

/// The pipe could not be used.
#[derive(Debug, thiserror::Error)]
pub enum PipeError {
    /// Another tray process owns the pipe.
    #[error("Huedra is already running")]
    AlreadyRunning,
    /// No tray process is running.
    #[error("Huedra is not running; start it first")]
    NotRunning,
    /// The pipe's server runs as another user.
    #[error("the program answering on Huedra's channel is not Huedra")]
    NotTrusted,
    /// A Windows call failed.
    #[error("the connection to Huedra failed")]
    Windows(#[from] windows::core::Error),
    /// The message was not valid.
    #[error(transparent)]
    Protocol(#[from] IpcError),
    /// The pipe thread could not start.
    #[error("could not start the connection thread")]
    Thread(#[from] std::io::Error),
}

/// A request from a client, waiting for the tray process's answer.
#[derive(Debug)]
pub struct PipeCall {
    /// What the client asked for.
    pub request: Request,
    reply: mpsc::Sender<Response>,
}

impl PipeCall {
    /// Sends the answer to the client.
    pub fn reply(self, response: Response) {
        let _ = self.reply.send(response);
    }
}

/// The server side, owned by the tray process. Requests arrive as
/// [`crate::event_loop::Event::Wake`] followed by [`Self::next_call`].
#[derive(Debug)]
pub struct PipeServer {
    calls: mpsc::Receiver<PipeCall>,
}

impl PipeServer {
    /// Creates the pipe and starts serving it on a background thread.
    ///
    /// # Errors
    /// [`PipeError::AlreadyRunning`] when another tray process owns it.
    pub fn start(waker: Waker) -> Result<Self, PipeError> {
        let sid = current_user_sid()?;
        let pipe = create_pipe(&pipe_name(&sid), &sid)?;
        let (sender, calls) = mpsc::channel();
        thread::Builder::new()
            .name("huedra-pipe".into())
            .spawn(move || serve(&pipe, &sender, waker))?;
        Ok(Self { calls })
    }

    /// The next waiting request, if any.
    pub fn next_call(&self) -> Option<PipeCall> {
        self.calls.try_recv().ok()
    }
}

/// Sends one request to the running tray process and returns its answer.
///
/// # Errors
/// [`PipeError::NotRunning`] when no tray process runs; other variants for
/// failures and untrusted servers.
pub fn call(request: &Request) -> Result<Response, PipeError> {
    let name = HSTRING::from(pipe_name(&current_user_sid()?));
    let mut attempts = 0;
    let pipe = loop {
        // SAFETY: opens an existing pipe. SECURITY_IDENTIFICATION stops a
        // server from acting with the client's identity.
        match unsafe {
            CreateFileW(
                &name,
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        } {
            Ok(handle) => break OwnedHandle(handle),
            Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() && attempts < 3 => {
                attempts += 1;
                // SAFETY: waits on a pipe name; no memory is shared.
                let _ = unsafe { WaitNamedPipeW(&name, BUSY_WAIT_MS) };
            }
            Err(e) if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() => {
                return Err(PipeError::NotRunning);
            }
            Err(e) => return Err(e.into()),
        }
    };
    verify_server(pipe.0)?;
    write_all(pipe.0, &ipc::encode(request)?)?;
    let bytes = read_message(pipe.0)?;
    Ok(ipc::decode(&bytes)?)
}

fn pipe_name(sid: &str) -> String {
    format!(r"\\.\pipe\Huedra-{sid}")
}

fn create_pipe(name: &str, sid: &str) -> Result<SendHandle, PipeError> {
    // Protected DACL with one entry: full access for this user only.
    let sddl = HSTRING::from(format!("D:P(A;;GA;;;{sid})"));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: parses a well-formed SDDL string; the result is freed by the guard.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            &sddl,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }?;
    let _descriptor = LocalMemory(descriptor.0);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    // SAFETY: `attributes` and the descriptor outlive the call.
    let handle = unsafe {
        CreateNamedPipeW(
            &HSTRING::from(name),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            BUFFER_BYTES,
            BUFFER_BYTES,
            0,
            Some(&attributes),
        )
    };
    if handle.is_invalid() {
        let error = windows::core::Error::from_thread();
        // With one instance allowed, a second creation fails with "busy";
        // with FILE_FLAG_FIRST_PIPE_INSTANCE it may also fail with "denied".
        if error.code() == ERROR_ACCESS_DENIED.to_hresult()
            || error.code() == ERROR_PIPE_BUSY.to_hresult()
        {
            return Err(PipeError::AlreadyRunning);
        }
        return Err(error.into());
    }
    Ok(SendHandle(OwnedHandle(handle)))
}

fn serve(pipe: &SendHandle, sender: &mpsc::Sender<PipeCall>, waker: Waker) {
    let handle = pipe.0.0;
    loop {
        // SAFETY: blocks until a client connects to our own pipe.
        let connected = unsafe { ConnectNamedPipe(handle, None) };
        let ok = match connected {
            Ok(()) => true,
            Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
        };
        if ok && !answer(handle, sender, waker) {
            return;
        }
        // SAFETY: ends this client's session on our pipe.
        unsafe {
            let _ = FlushFileBuffers(handle);
            let _ = DisconnectNamedPipe(handle);
        }
        if !ok {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Answers one client. Returns `false` when the tray process has gone.
fn answer(pipe: HANDLE, sender: &mpsc::Sender<PipeCall>, waker: Waker) -> bool {
    let response = match read_message(pipe)
        .map_err(PipeError::from)
        .and_then(|b| Ok(ipc::decode::<Request>(&b)?))
    {
        Ok(request) => {
            let (reply, answer) = mpsc::channel();
            if sender.send(PipeCall { request, reply }).is_err() {
                return false;
            }
            waker.wake();
            answer
                .recv_timeout(REPLY_TIMEOUT)
                .unwrap_or_else(|_| Response::Failed {
                    message: "Huedra did not answer in time.".into(),
                })
        }
        Err(error) => Response::Failed {
            message: error.to_string(),
        },
    };
    if let Ok(bytes) = ipc::encode(&response) {
        let _ = write_all(pipe, &bytes);
    }
    true
}

fn read_message(pipe: HANDLE) -> windows::core::Result<Vec<u8>> {
    let mut message = Vec::new();
    let mut chunk = [0_u8; 4096];
    while !message.ends_with(b"\n") {
        let mut read = 0_u32;
        // SAFETY: `chunk` is a valid buffer; `read` receives the count.
        unsafe { ReadFile(pipe, Some(&mut chunk), Some(&mut read), None) }?;
        if read == 0 {
            break;
        }
        message.extend_from_slice(&chunk[..read as usize]);
        if message.len() > MAX_MESSAGE_BYTES {
            break;
        }
    }
    Ok(message)
}

fn write_all(pipe: HANDLE, mut bytes: &[u8]) -> windows::core::Result<()> {
    while !bytes.is_empty() {
        let mut written = 0_u32;
        // SAFETY: `bytes` is a valid buffer; `written` receives the count.
        unsafe { WriteFile(pipe, Some(bytes), Some(&mut written), None) }?;
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

/// The `TOKEN_USER` of a process, in a buffer aligned for its pointers.
struct UserToken(Vec<u64>);

impl UserToken {
    fn of(process: HANDLE) -> windows::core::Result<Self> {
        let mut token = HANDLE::default();
        // SAFETY: requests query access to the token only.
        unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }?;
        let token = OwnedHandle(token);
        let mut needed = 0_u32;
        // SAFETY: a size query; failure with "insufficient buffer" is expected.
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut needed) };
        let mut buffer = vec![0_u64; (needed as usize).div_ceil(8)];
        // SAFETY: `buffer` holds at least `needed` bytes, 8-byte aligned.
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                needed,
                &mut needed,
            )
        }?;
        Ok(Self(buffer))
    }

    fn sid(&self) -> PSID {
        // SAFETY: the buffer was filled with a TOKEN_USER by Windows and is
        // aligned for it; the SID points into the same buffer.
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
}

fn current_user_sid() -> windows::core::Result<String> {
    // SAFETY: a pseudo-handle that needs no closing.
    let token = UserToken::of(unsafe { GetCurrentProcess() })?;
    let mut text = PWSTR::null();
    // SAFETY: converts a valid SID; the string is freed by the guard.
    unsafe { ConvertSidToStringSidW(token.sid(), &mut text) }?;
    let _text = LocalMemory(text.0.cast());
    // SAFETY: Windows returned a null-terminated string.
    Ok(unsafe { text.to_string() }.unwrap_or_default())
}

fn verify_server(pipe: HANDLE) -> Result<(), PipeError> {
    let mut pid = 0_u32;
    // SAFETY: `pid` is a valid out-parameter.
    unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) }?;
    // SAFETY: requests the limited query right only.
    let server =
        OwnedHandle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?);
    let theirs = UserToken::of(server.0)?;
    // SAFETY: a pseudo-handle that needs no closing.
    let ours = UserToken::of(unsafe { GetCurrentProcess() })?;
    // SAFETY: both SIDs are valid for the lifetime of their buffers.
    unsafe { EqualSid(theirs.sid(), ours.sid()) }.map_err(|_| PipeError::NotTrusted)
}
