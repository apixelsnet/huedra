//! NVIDIA RTX HDR per program, through the driver's settings database
//! (DRS) in `nvapi64.dll`, which the NVIDIA driver installs in System32.
//! Without an NVIDIA driver nothing opens and nothing changes. Function ids
//! and structure layouts are from NVIDIA's public NVAPI headers
//! (`nvapi_interface.h`, `nvapi.h`).
//! Feature doc: `docs/features/04-game-library.md`.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::OnceLock;

use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::core::{PCSTR, w};

/// The driver setting that turns RTX HDR on (1) or off (0) in a profile.
pub const RTX_HDR: u32 = 0x00DD_48FB;

const TEXT_MAX: usize = 2048;
type Text = [u16; TEXT_MAX];
type Handle = *mut c_void;

const OK: i32 = 0;
const SETTING_NOT_FOUND: i32 = -160;
const PROFILE_NOT_FOUND: i32 = -163;
const EXECUTABLE_NOT_FOUND: i32 = -166;

/// `NVDRS_APPLICATION_V4`.
#[repr(C)]
struct Application {
    version: u32,
    is_predefined: u32,
    app_name: Text,
    friendly_name: Text,
    launcher: Text,
    file_in_folder: Text,
    flags: u32,
    command_line: Text,
}

/// `NVDRS_PROFILE_V1`.
#[repr(C)]
struct Profile {
    version: u32,
    name: Text,
    gpu_support: u32,
    is_predefined: u32,
    apps: u32,
    settings: u32,
}

/// `NVDRS_SETTING_V1`; each value union is 4,100 bytes (the binary form).
#[repr(C)]
struct Setting {
    version: u32,
    name: Text,
    id: u32,
    kind: u32,
    location: u32,
    is_current_predefined: u32,
    is_predefined_valid: u32,
    predefined: [u32; 1025],
    current: [u32; 1025],
}

const _: () = assert!(size_of::<Application>() == 20_492, "NVDRS_APPLICATION_V4");
const _: () = assert!(size_of::<Profile>() == 4_116, "NVDRS_PROFILE_V1");
const _: () = assert!(size_of::<Setting>() == 12_320, "NVDRS_SETTING_V1");

/// `MAKE_NVAPI_VERSION`: the structure size and its version.
fn version<T>(number: u32) -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(0) | (number << 16)
}

/// Text in NVAPI's fixed-size form, cut to fit.
fn text(value: &str) -> Box<Text> {
    let mut out = Box::new([0_u16; TEXT_MAX]);
    for (slot, unit) in out.iter_mut().zip(value.encode_utf16().take(TEXT_MAX - 1)) {
        *slot = unit;
    }
    out
}

impl Application {
    fn new(app_name: &str, friendly_name: &str) -> Box<Self> {
        Box::new(Self {
            version: version::<Self>(4),
            is_predefined: 0,
            app_name: *text(app_name),
            friendly_name: *text(friendly_name),
            launcher: [0; TEXT_MAX],
            file_in_folder: [0; TEXT_MAX],
            flags: 0,
            command_line: [0; TEXT_MAX],
        })
    }
}

impl Setting {
    fn new(id: u32, value: u32) -> Box<Self> {
        let mut current = [0; 1025];
        current[0] = value;
        Box::new(Self {
            version: version::<Self>(1),
            name: [0; TEXT_MAX],
            id,
            // NVDRS_DWORD_TYPE
            kind: 0,
            // NVDRS_CURRENT_PROFILE_LOCATION
            location: 0,
            is_current_predefined: 0,
            is_predefined_valid: 0,
            predefined: [0; 1025],
            current,
        })
    }
}

struct Api {
    create_session: unsafe extern "C" fn(*mut Handle) -> i32,
    destroy_session: unsafe extern "C" fn(Handle) -> i32,
    load: unsafe extern "C" fn(Handle) -> i32,
    save: unsafe extern "C" fn(Handle) -> i32,
    find_app: unsafe extern "C" fn(Handle, *const u16, *mut Handle, *mut Application) -> i32,
    find_profile: unsafe extern "C" fn(Handle, *const u16, *mut Handle) -> i32,
    create_profile: unsafe extern "C" fn(Handle, *mut Profile, *mut Handle) -> i32,
    create_app: unsafe extern "C" fn(Handle, Handle, *mut Application) -> i32,
    get_setting: unsafe extern "C" fn(Handle, Handle, u32, *mut Setting) -> i32,
    set_setting: unsafe extern "C" fn(Handle, Handle, *mut Setting) -> i32,
    delete_setting: unsafe extern "C" fn(Handle, Handle, u32) -> i32,
    profile_info: unsafe extern "C" fn(Handle, Handle, *mut Profile) -> i32,
    delete_profile: unsafe extern "C" fn(Handle, Handle) -> i32,
    // Drivers from R610 on answer the documented GetSetting and SetSetting
    // with "setting not found" for newer settings such as RTX HDR, and take
    // these undocumented successors instead (ids and signatures as other
    // tools that set RTX HDR use them; the extra arguments stay zero).
    get_setting_new:
        Option<unsafe extern "C" fn(Handle, Handle, u32, *mut Setting, *mut u32) -> i32>,
    set_setting_new: Option<unsafe extern "C" fn(Handle, Handle, *mut Setting, u32, u32) -> i32>,
}

impl Api {
    /// GetSetting through the newer entry point when the driver has it.
    ///
    /// # Safety
    /// Valid session and profile handles; `setting` has its version set.
    unsafe fn get(&self, session: Handle, profile: Handle, id: u32, setting: *mut Setting) -> i32 {
        match self.get_setting_new {
            Some(get) => {
                let mut extra = 0_u32;
                // SAFETY: as the caller vouches; `extra` lives for the call.
                unsafe { get(session, profile, id, setting, &raw mut extra) }
            }
            // SAFETY: as the caller vouches.
            None => unsafe { (self.get_setting)(session, profile, id, setting) },
        }
    }

    /// SetSetting through the newer entry point when the driver has it.
    ///
    /// # Safety
    /// Valid session and profile handles; `setting` has its version set.
    unsafe fn set(&self, session: Handle, profile: Handle, setting: *mut Setting) -> i32 {
        match self.set_setting_new {
            // SAFETY: as the caller vouches.
            Some(set) => unsafe { set(session, profile, setting, 0, 0) },
            // SAFETY: as the caller vouches.
            None => unsafe { (self.set_setting)(session, profile, setting) },
        }
    }
}

type QueryInterface = unsafe extern "C" fn(u32) -> *mut c_void;

/// Looks up one NVAPI function by id and gives it the function type `T`.
///
/// # Safety
/// `T` must be the function's real signature, as a function pointer type.
unsafe fn function<T: Copy>(query: QueryInterface, id: u32) -> Option<T> {
    // SAFETY: nvapi_QueryInterface takes any id and returns null for
    // unknown ones.
    let found = unsafe { query(id) };
    if found.is_null() {
        return None;
    }
    debug_assert_eq!(size_of::<T>(), size_of::<*mut c_void>(), "function pointer");
    // SAFETY: a non-null function pointer of the same size; the caller
    // vouches for the signature.
    Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&found) })
}

fn load_api() -> Option<Api> {
    // SAFETY: loads the NVIDIA driver's DLL from System32 only. It stays
    // loaded for the life of the process, as NVIDIA advises.
    let module =
        unsafe { LoadLibraryExW(w!("nvapi64.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.ok()?;
    // SAFETY: a null-terminated ASCII name in a loaded module.
    let query = unsafe { GetProcAddress(module, PCSTR(c"nvapi_QueryInterface".as_ptr().cast())) }?;
    // SAFETY: nvapi_QueryInterface's documented signature.
    let query: QueryInterface = unsafe {
        std::mem::transmute_copy::<unsafe extern "system" fn() -> isize, QueryInterface>(&query)
    };
    // SAFETY: the ids and signatures are those of nvapi_interface.h and
    // nvapi.h; NVAPI_INTERFACE is the C calling convention.
    unsafe {
        let initialize: unsafe extern "C" fn() -> i32 = function(query, 0x0150_E828)?;
        if initialize() != OK {
            return None;
        }
        Some(Api {
            create_session: function(query, 0x0694_D52E)?,
            destroy_session: function(query, 0xDAD9_CFF8)?,
            load: function(query, 0x375D_BD6B)?,
            save: function(query, 0xFCBC_7E14)?,
            find_app: function(query, 0xEEE5_66B2)?,
            find_profile: function(query, 0x7E4A_9A0B)?,
            create_profile: function(query, 0xCC17_6068)?,
            create_app: function(query, 0x4347_A9DE)?,
            get_setting: function(query, 0x73BF_8338)?,
            set_setting: function(query, 0x577D_D202)?,
            delete_setting: function(query, 0xE4A2_6362)?,
            profile_info: function(query, 0x61CD_6FD6)?,
            delete_profile: function(query, 0x1709_3206)?,
            get_setting_new: function(query, 0xEA99_498D),
            set_setting_new: function(query, 0x8A2C_F5F5),
        })
    }
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(load_api).as_ref()
}

/// The driver refused a request; the number is NVAPI's status.
#[derive(Debug, thiserror::Error)]
#[error("the NVIDIA driver refused the change (status {0})")]
pub struct NvError(pub i32);

fn check(status: i32) -> Result<(), NvError> {
    if status == OK {
        Ok(())
    } else {
        Err(NvError(status))
    }
}

/// An open copy of the driver's settings. Changes take effect once saved,
/// for programs started after that.
pub struct Session {
    api: &'static Api,
    handle: Handle,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: a session this value created, destroyed once.
        let _ = unsafe { (self.api.destroy_session)(self.handle) };
    }
}

impl Session {
    /// Opens the driver's settings; `None` without an NVIDIA driver.
    pub fn open() -> Option<Self> {
        let api = api()?;
        let mut handle: Handle = std::ptr::null_mut();
        // SAFETY: the out pointer is valid for the call.
        if unsafe { (api.create_session)(&raw mut handle) } != OK || handle.is_null() {
            return None;
        }
        let session = Self { api, handle };
        // SAFETY: a valid session.
        (unsafe { (api.load)(handle) } == OK).then_some(session)
    }

    /// The profile a program (by file name, such as `game.exe`) belongs
    /// to, or `None` when no profile lists it.
    fn profile_of(&self, program: &str) -> Result<Option<Handle>, NvError> {
        let name = text(program);
        let mut app = Application::new("", "");
        let mut profile: Handle = std::ptr::null_mut();
        // SAFETY: the name buffer has NVAPI's full size; the out pointers
        // are valid for the call.
        let status = unsafe {
            (self.api.find_app)(self.handle, name.as_ptr(), &raw mut profile, &raw mut *app)
        };
        match status {
            OK => Ok(Some(profile)),
            EXECUTABLE_NOT_FOUND => Ok(None),
            other => Err(NvError(other)),
        }
    }

    /// RTX HDR as the program's own profile sets it: `Ok(None)` when it
    /// does not set it.
    ///
    /// # Errors
    /// [`NvError`] when the driver refuses the request.
    pub fn rtx_hdr(&self, program: &str) -> Result<Option<u32>, NvError> {
        let Some(profile) = self.profile_of(program)? else {
            return Ok(None);
        };
        let mut setting = Setting::new(0, 0);
        // SAFETY: a valid session and profile; the structure has its
        // version set.
        let status = unsafe {
            self.api
                .get(self.handle, profile, RTX_HDR, &raw mut *setting)
        };
        match status {
            // Only a value set in the program's own profile counts.
            OK => Ok((setting.location == 0).then_some(setting.current[0])),
            SETTING_NOT_FOUND => Ok(None),
            other => Err(NvError(other)),
        }
    }

    /// Sets RTX HDR for a program (by file name), or removes the value with
    /// `None`. A program no profile lists is added to a profile named
    /// `profile_name` (made when missing).
    ///
    /// # Errors
    /// [`NvError`] when the driver refuses a step.
    pub fn set_rtx_hdr(
        &mut self,
        program: &str,
        profile_name: &str,
        value: Option<u32>,
    ) -> Result<(), NvError> {
        let profile = match (self.profile_of(program)?, value) {
            (Some(profile), _) => profile,
            (None, None) => return Ok(()),
            (None, Some(_)) => {
                let name = text(profile_name);
                let mut profile: Handle = std::ptr::null_mut();
                // SAFETY: as in `profile_of`.
                let status = unsafe {
                    (self.api.find_profile)(self.handle, name.as_ptr(), &raw mut profile)
                };
                if status == PROFILE_NOT_FOUND {
                    let mut info = Box::new(Profile {
                        version: version::<Profile>(1),
                        name: *name,
                        gpu_support: 0,
                        is_predefined: 0,
                        apps: 0,
                        settings: 0,
                    });
                    // SAFETY: the structure has its version set; the out
                    // pointer is valid.
                    check(unsafe {
                        (self.api.create_profile)(self.handle, &raw mut *info, &raw mut profile)
                    })?;
                } else {
                    check(status)?;
                }
                let mut app = Application::new(program, profile_name);
                // SAFETY: a valid session and profile; the structure has its
                // version set.
                check(unsafe { (self.api.create_app)(self.handle, profile, &raw mut *app) })?;
                profile
            }
        };
        match value {
            Some(value) => {
                let mut setting = Setting::new(RTX_HDR, value);
                // SAFETY: as above.
                check(unsafe { self.api.set(self.handle, profile, &raw mut *setting) })
            }
            None => {
                // SAFETY: a valid session and profile.
                match unsafe { (self.api.delete_setting)(self.handle, profile, RTX_HDR) } {
                    OK | SETTING_NOT_FOUND => Ok(()),
                    other => Err(NvError(other)),
                }
            }
        }
    }

    /// Deletes the profile of a program (by file name) when it is one
    /// Huedra made for it: named with `suffix` and listing only that
    /// program. Returns whether one was deleted.
    ///
    /// # Errors
    /// [`NvError`] when the driver refuses a step.
    pub fn remove_own_profile(&mut self, program: &str, suffix: &str) -> Result<bool, NvError> {
        let Some(profile) = self.profile_of(program)? else {
            return Ok(false);
        };
        let mut info = Box::new(Profile {
            version: version::<Profile>(1),
            name: [0; TEXT_MAX],
            gpu_support: 0,
            is_predefined: 0,
            apps: 0,
            settings: 0,
        });
        // SAFETY: a valid session and profile; the structure has its version
        // set.
        check(unsafe { (self.api.profile_info)(self.handle, profile, &raw mut *info) })?;
        let len = info.name.iter().position(|c| *c == 0).unwrap_or(TEXT_MAX);
        let name = String::from_utf16_lossy(&info.name[..len]);
        if info.is_predefined != 0 || info.apps != 1 || !name.ends_with(suffix) {
            return Ok(false);
        }
        // SAFETY: a valid session and profile.
        check(unsafe { (self.api.delete_profile)(self.handle, profile) })?;
        Ok(true)
    }

    /// Writes the changes to the driver.
    ///
    /// # Errors
    /// [`NvError`] when the driver refuses.
    pub fn save(&self) -> Result<(), NvError> {
        // SAFETY: a valid session.
        check(unsafe { (self.api.save)(self.handle) })
    }
}

/// Whether an NVIDIA driver answers, so RTX HDR can be set. `false` on
/// PCs with other graphics (AMD, Intel), where RTX HDR does not exist.
pub fn available() -> bool {
    api().is_some()
}
