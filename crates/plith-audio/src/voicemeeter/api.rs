//! Typed table of the Remote API exports and safe wrappers around them.

use std::ffi::{CString, c_char};
use std::sync::Mutex;

use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::core::PCSTR;

use super::decode::decode_label;
use super::dll::load_remote_dll;

type NoArgFn = unsafe extern "system" fn() -> i32;
type GetFloatFn = unsafe extern "system" fn(*const c_char, *mut f32) -> i32;
type GetStringFn = unsafe extern "system" fn(*const c_char, *mut u8) -> i32;
type SetFloatFn = unsafe extern "system" fn(*const c_char, f32) -> i32;
type GetTypeFn = unsafe extern "system" fn(*mut i32) -> i32;
type GetLevelFn = unsafe extern "system" fn(i32, i32, *mut f32) -> i32;

/// Size the API requires for string results.
const STRING_BUFFER: usize = 512;

/// Resolved exports plus the module handle that keeps them valid.
pub(super) struct Api {
    module: HMODULE,
    login: NoArgFn,
    logout: NoArgFn,
    is_dirty: NoArgFn,
    get_float: GetFloatFn,
    get_string: GetStringFn,
    set_float: SetFloatFn,
    get_type: GetTypeFn,
    get_level: GetLevelFn,
    /// The engine API is not documented as thread safe, so every call takes this lock.
    gate: Mutex<()>,
}

// SAFETY: the module handle and function pointers are process wide and immutable; calls are
// serialized through `gate`.
unsafe impl Send for Api {}
// SAFETY: see `Send`; shared access only reaches the library through `gate`.
unsafe impl Sync for Api {}

/// Resolves one export and casts it to its function type.
macro_rules! resolve {
    ($module:expr, $name:literal, $ty:ty) => {{
        // SAFETY: `$module` is a loaded module and the name is a NUL terminated literal.
        let proc = unsafe { GetProcAddress($module, PCSTR(concat!($name, "\0").as_ptr())) }
            .ok_or_else(|| format!("{} is missing from the Voicemeeter DLL", $name))?;
        // SAFETY: the Remote API documents this export with exactly the signature of `$ty`
        // (`long` is 32 bit on Windows, calling convention stdcall, `extern "system"`).
        unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(proc) }
    }};
}

impl Api {
    /// Locates and loads the DLL and resolves every export.
    pub(super) fn load() -> Result<Self, String> {
        let module = load_remote_dll()?;
        match Self::resolve(module) {
            Ok(api) => Ok(api),
            Err(reason) => {
                // SAFETY: `module` was just loaded by us and nothing references it yet.
                unsafe {
                    let _ = FreeLibrary(module);
                }
                Err(reason)
            }
        }
    }

    fn resolve(module: HMODULE) -> Result<Self, String> {
        Ok(Self {
            module,
            login: resolve!(module, "VBVMR_Login", NoArgFn),
            logout: resolve!(module, "VBVMR_Logout", NoArgFn),
            is_dirty: resolve!(module, "VBVMR_IsParametersDirty", NoArgFn),
            get_float: resolve!(module, "VBVMR_GetParameterFloat", GetFloatFn),
            get_string: resolve!(module, "VBVMR_GetParameterStringA", GetStringFn),
            set_float: resolve!(module, "VBVMR_SetParameterFloat", SetFloatFn),
            get_type: resolve!(module, "VBVMR_GetVoicemeeterType", GetTypeFn),
            get_level: resolve!(module, "VBVMR_GetLevel", GetLevelFn),
            gate: Mutex::new(()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `VBVMR_Login`: 0 engine already running, 1 engine launched, negative on failure.
    pub(super) fn login(&self) -> i32 {
        let _gate = self.lock();
        // SAFETY: resolved export with no arguments.
        unsafe { (self.login)() }
    }

    /// `VBVMR_Logout`.
    pub(super) fn logout(&self) -> i32 {
        let _gate = self.lock();
        // SAFETY: resolved export with no arguments.
        unsafe { (self.logout)() }
    }

    /// `VBVMR_IsParametersDirty`: 0 clean, 1 dirty, negative on error.
    pub(super) fn is_dirty(&self) -> i32 {
        let _gate = self.lock();
        // SAFETY: resolved export with no arguments.
        unsafe { (self.is_dirty)() }
    }

    /// `VBVMR_GetVoicemeeterType` as the raw type code.
    pub(super) fn voicemeeter_type(&self) -> Option<i32> {
        let _gate = self.lock();
        let mut code = 0i32;
        // SAFETY: `code` is a valid out pointer for the call.
        let rc = unsafe { (self.get_type)(&raw mut code) };
        (rc == 0).then_some(code)
    }

    /// `VBVMR_GetParameterFloat`; `None` when the call fails.
    pub(super) fn get_float(&self, name: &str) -> Option<f32> {
        let name = CString::new(name).ok()?;
        let _gate = self.lock();
        let mut value = 0f32;
        // SAFETY: `name` is NUL terminated and `value` is a valid out pointer.
        let rc = unsafe { (self.get_float)(name.as_ptr(), &raw mut value) };
        (rc == 0).then_some(value)
    }

    /// `VBVMR_GetParameterStringA` decoded to a `String`; `None` when the call fails.
    pub(super) fn get_string(&self, name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let _gate = self.lock();
        let mut buffer = [0u8; STRING_BUFFER];
        // SAFETY: `name` is NUL terminated; the API requires a buffer of at least 512 bytes
        // and `buffer` is exactly that.
        let rc = unsafe { (self.get_string)(name.as_ptr(), buffer.as_mut_ptr()) };
        (rc == 0).then(|| decode_label(&buffer))
    }

    /// `VBVMR_SetParameterFloat`; true on success. The only write the client performs.
    pub(super) fn set_float(&self, name: &str, value: f32) -> bool {
        let Ok(name) = CString::new(name) else {
            return false;
        };
        let _gate = self.lock();
        // SAFETY: `name` is NUL terminated.
        unsafe { (self.set_float)(name.as_ptr(), value) == 0 }
    }

    /// `VBVMR_GetLevel` for one channel; `None` when the call fails.
    pub(super) fn level(&self, kind: i32, channel: i32) -> Option<f32> {
        let _gate = self.lock();
        let mut value = 0f32;
        // SAFETY: `value` is a valid out pointer for the call.
        let rc = unsafe { (self.get_level)(kind, channel, &raw mut value) };
        (rc == 0).then_some(value)
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        // SAFETY: we loaded `module` and no function pointer outlives this struct.
        unsafe {
            let _ = FreeLibrary(self.module);
        }
    }
}
