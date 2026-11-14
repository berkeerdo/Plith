//! Core Audio calls: COM apartment, enumeration, and the attached endpoint. Everything here runs
//! on the service thread only.

use std::sync::mpsc::Sender;

use windows::Win32::Devices::FunctionDiscovery::{
    PKEY_Device_FriendlyName, PKEY_DeviceInterface_FriendlyName,
};
use windows::Win32::Foundation::{E_FAIL, PROPERTYKEY};
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback};
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    eMultimedia, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, STGM_READ,
};
use windows::core::{Error, PCWSTR, Result};

use super::notify::{Msg, VolumeEvents};
use super::{AttachedState, Endpoint};

/// Holds the thread's COM initialization; dropping it uninitializes. Declare it before any COM
/// object so those are released first.
pub(crate) struct ComGuard;

impl ComGuard {
    /// Joins the multithreaded apartment.
    pub(crate) fn init() -> Result<Self> {
        // SAFETY: plain apartment initialization on the calling thread, balanced by `Drop`.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: balances the successful `CoInitializeEx` in `init`.
        unsafe { CoUninitialize() };
    }
}

/// Creates the device enumerator.
pub(crate) fn create_enumerator() -> Result<IMMDeviceEnumerator> {
    // SAFETY: `MMDeviceEnumerator` is the documented class id for `IMMDeviceEnumerator`.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn device_id(device: &IMMDevice) -> Result<String> {
    // SAFETY: `GetId` returns a CoTaskMem string that is read once and freed once.
    unsafe {
        let raw = device.GetId()?;
        let text = raw.to_string();
        CoTaskMemFree(Some(raw.0.cast_const().cast()));
        text.map_err(|_| Error::from(E_FAIL))
    }
}

fn read_name(device: &IMMDevice, key: &PROPERTYKEY) -> Option<String> {
    // SAFETY: the store and the variant are owned values released on drop; `key` is valid.
    let value = unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        store.GetValue(key).ok()?
    };
    let text = value.to_string();
    (!text.is_empty()).then_some(text)
}

/// Reads one endpoint completely or not at all. The id and the endpoint name are required;
/// the device description is optional and falls back to empty.
fn read_endpoint(device: &IMMDevice) -> Option<Endpoint> {
    let id = device_id(device).ok()?;
    let friendly_name = read_name(device, &PKEY_Device_FriendlyName)?;
    let device_name = read_name(device, &PKEY_DeviceInterface_FriendlyName).unwrap_or_default();
    Some(Endpoint {
        id,
        friendly_name,
        device_name,
    })
}

/// Every active render endpoint that could be read completely; empty when the stack fails.
pub(crate) fn list_active(en: &IMMDeviceEnumerator) -> Vec<Endpoint> {
    // SAFETY: enumeration on a live enumerator; results are owned interface pointers.
    unsafe {
        let Ok(devices) = en.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
            return Vec::new();
        };
        let count = devices.GetCount().unwrap_or(0);
        (0..count)
            .filter_map(|i| devices.Item(i).ok())
            .filter_map(|device| read_endpoint(&device))
            .collect()
    }
}

/// Id of the default render endpoint (Multimedia role); `None` when there is none.
pub(crate) fn default_id(en: &IMMDeviceEnumerator) -> Option<String> {
    // SAFETY: lookup on a live enumerator.
    let device = unsafe { en.GetDefaultAudioEndpoint(eRender, eMultimedia) }.ok()?;
    device_id(&device).ok()
}

/// The default endpoint of `flow` for `role`, read completely; `None` when there is none.
pub(crate) fn default_endpoint(
    en: &IMMDeviceEnumerator,
    flow: EDataFlow,
    role: ERole,
) -> Option<Endpoint> {
    // SAFETY: lookup on a live enumerator.
    let device = unsafe { en.GetDefaultAudioEndpoint(flow, role) }.ok()?;
    read_endpoint(&device)
}

/// The endpoint volume interface of one device plus the callback registered on it.
pub(crate) struct Attachment {
    /// Endpoint id this attachment follows.
    pub(crate) id: String,
    /// Identifies this attachment in volume messages.
    pub(crate) generation: u64,
    pub(super) volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl Attachment {
    /// Opens `id`, registers the volume callback and reads the current state.
    pub(crate) fn attach(
        en: &IMMDeviceEnumerator,
        id: &str,
        generation: u64,
        tx: &Sender<Msg>,
    ) -> Result<(Self, AttachedState)> {
        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `wide` is NUL terminated and outlives the call; the interfaces are owned.
        let volume = unsafe {
            let device = en.GetDevice(PCWSTR(wide.as_ptr()))?;
            device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?
        };
        let callback: IAudioEndpointVolumeCallback = VolumeEvents {
            tx: tx.clone(),
            generation,
        }
        .into();
        // SAFETY: `callback` stays alive until `detach` unregisters it.
        unsafe { volume.RegisterControlChangeNotify(&callback) }?;
        let attachment = Self {
            id: id.to_owned(),
            generation,
            volume,
            callback,
        };
        if let Some(state) = attachment.read_state() {
            return Ok((attachment, state));
        }
        attachment.detach();
        Err(Error::from(E_FAIL))
    }

    /// Current volume and mute; `None` when the endpoint no longer answers.
    pub(crate) fn read_state(&self) -> Option<AttachedState> {
        // SAFETY: plain getters on a live interface.
        let (volume, muted) = unsafe {
            (
                self.volume.GetMasterVolumeLevelScalar().ok()?,
                self.volume.GetMute().ok()?.as_bool(),
            )
        };
        Some(AttachedState {
            id: self.id.clone(),
            volume,
            muted,
        })
    }

    /// Unregisters the callback. Called from the service thread with no lock held: the call
    /// waits for an in-flight callback, and callbacks never wait for anything.
    pub(crate) fn detach(self) {
        // SAFETY: `callback` is the one registered in `attach`. A failure leaves nothing to do.
        let _ = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) };
    }
}
