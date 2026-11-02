//! Starting a program for `huedra launch` and waiting for it. The program
//! runs inside a job object, so the programs it starts in turn can be
//! followed after it exits (games that open through a small launcher).
//! Arguments go to `CreateProcessW` through `std::process`, never through
//! `cmd.exe`. Feature doc: `docs/features/06-launch-with-hdr.md`.

use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle as _;
use std::os::windows::process::CommandExt as _;
use std::process::{Child, ExitStatus};
use std::time::Duration;

use huedra_core::paths::NormPath;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
    JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject,
};
use windows::Win32::System::Threading::{
    CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};
use windows::core::PCWSTR;

use crate::handle::OwnedHandle;
use crate::process;

/// How often the job is checked once the started program has exited.
const POLL: Duration = Duration::from_secs(1);
/// Most process ids read from the job at once.
const MAX_IDS: usize = 256;

/// A program created for `huedra launch` that has not run yet. It stays
/// suspended until [`Suspended::resume`], so a program that cannot be
/// started is found before HDR is turned on for it. Dropped without being
/// resumed, it is ended.
pub struct Suspended {
    child: Option<Child>,
    job: Option<OwnedHandle>,
}

impl Suspended {
    /// Creates `program` with `args`, suspended and inside a new job (so
    /// nothing it starts escapes). When the job cannot be created, only the
    /// program itself is waited for later.
    ///
    /// # Errors
    /// The error from creating the program, such as a missing file.
    pub fn create(program: &str, args: &[String]) -> std::io::Result<Self> {
        let job = new_job();
        let mut command = std::process::Command::new(program);
        command.args(args).creation_flags(CREATE_SUSPENDED.0);
        let child = command.spawn()?;
        let process = HANDLE(child.as_raw_handle());
        let assigned = job.as_ref().is_some_and(|job| {
            // SAFETY: both handles are open; the child's stays valid while
            // `child` lives.
            unsafe { AssignProcessToJobObject(job.0, process) }.is_ok()
        });
        Ok(Self {
            child: Some(child),
            job: job.filter(|_| assigned),
        })
    }

    /// Lets the program run.
    ///
    /// # Errors
    /// The program could not be resumed; it has been ended.
    pub fn resume(mut self) -> std::io::Result<Launched> {
        let Some(mut child) = self.child.take() else {
            return Err(std::io::Error::other("the program has already run"));
        };
        if !resume(child.id()) {
            let _ = child.kill();
            return Err(std::io::Error::other("the program could not be resumed"));
        }
        Ok(Launched {
            child,
            job: self.job.take(),
        })
    }
}

impl Drop for Suspended {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// A program started by `huedra launch`.
pub struct Launched {
    child: Child,
    job: Option<OwnedHandle>,
}

impl Launched {
    /// The full path of the program Windows started.
    pub fn path(&self) -> Option<String> {
        process::image_path(self.child.id())
    }

    /// Waits for the program, then for every program it started whose file
    /// lies in `folder`. Returns the program's own exit status.
    ///
    /// # Errors
    /// The error from waiting for the program.
    pub fn wait(mut self, folder: &NormPath) -> std::io::Result<ExitStatus> {
        let status = self.child.wait()?;
        if let Some(job) = &self.job {
            while job_pids(job)
                .into_iter()
                .filter_map(process::image_path)
                .any(|path| NormPath::new(&path).is_within(folder))
            {
                std::thread::sleep(POLL);
            }
        }
        Ok(status)
    }
}

/// A job whose programs may leave it when they ask to (some launchers
/// require that), and which ends nothing when Huedra exits.
fn new_job() -> Option<OwnedHandle> {
    // SAFETY: an unnamed job with default security.
    let job = OwnedHandle(unsafe { CreateJobObjectW(None, PCWSTR::null()) }.ok()?);
    // The breakaway flag is accepted only in the extended structure.
    let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_BREAKAWAY_OK,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `limits` is the structure this information class expects.
    unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast::<c_void>(),
            u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).ok()?,
        )
    }
    .ok()?;
    Some(job)
}

/// Resumes the threads of a process created suspended (it has one).
fn resume(pid: u32) -> bool {
    // SAFETY: takes a snapshot of the thread list only.
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }) else {
        return false;
    };
    let snapshot = OwnedHandle(snapshot);
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).unwrap_or(0),
        ..Default::default()
    };
    let mut resumed = false;
    // SAFETY: `entry` has its size set, as the function requires.
    let mut more = unsafe { Thread32First(snapshot.0, &mut entry) }.is_ok();
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: requests only the right to resume the thread.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) };
            if let Ok(thread) = thread {
                let thread = OwnedHandle(thread);
                // SAFETY: resumes a thread of the process we created.
                resumed |= unsafe { ResumeThread(thread.0) } != u32::MAX;
            }
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot.0, &mut entry) }.is_ok();
    }
    resumed
}

/// The process-id list as Windows writes it, with room for [`MAX_IDS`].
#[repr(C)]
struct IdList {
    _assigned: u32,
    listed: u32,
    ids: [usize; MAX_IDS],
}

/// The processes in the job now.
fn job_pids(job: &OwnedHandle) -> Vec<u32> {
    let mut list = IdList {
        _assigned: 0,
        listed: 0,
        ids: [0; MAX_IDS],
    };
    let bytes = u32::try_from(size_of::<IdList>()).unwrap_or(0);
    // SAFETY: `list` has the layout of JOBOBJECT_BASIC_PROCESS_ID_LIST with
    // room for MAX_IDS ids, and `bytes` is its size. When more processes
    // exist the call fails but still fills what fits.
    let _ = unsafe {
        QueryInformationJobObject(
            Some(job.0),
            JobObjectBasicProcessIdList,
            (&raw mut list).cast::<c_void>(),
            bytes,
            None,
        )
    };
    let listed = (list.listed as usize).min(MAX_IDS);
    list.ids[..listed]
        .iter()
        .filter_map(|&id| u32::try_from(id).ok())
        .collect()
}
