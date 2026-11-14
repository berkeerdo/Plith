//! COM callbacks. They fire on threads Core Audio owns, so they do one thing: post a message
//! to the service thread and return. No lock, no COM call back into Core Audio, no waiting.

// The `#[implement]` expansion trips these pedantic lints; the code it generates is not ours.
#![allow(clippy::ref_as_ptr, clippy::inline_always)]

use std::sync::mpsc::Sender;

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole, IMMNotificationClient,
    IMMNotificationClient_Impl, eMultimedia, eRender,
};
use windows::core::{PCWSTR, Result, implement};

/// A message for the service thread.
pub(super) enum Msg {
    /// The device list, the default or a device state changed: re-read and re-resolve.
    Refresh,
    /// The attached endpoint reported a volume or mute change.
    Volume {
        /// Attachment the callback was registered for; stale ones are ignored.
        generation: u64,
        /// Master volume scalar.
        volume: f32,
        /// Mute flag.
        muted: bool,
    },
    /// Shut the thread down.
    Stop,
}

/// Device-level events: default changed, state changed, added, removed.
#[implement(IMMNotificationClient)]
pub(super) struct DeviceEvents {
    pub(super) tx: Sender<Msg>,
}

impl DeviceEvents {
    #[allow(clippy::unnecessary_wraps)]
    fn post(&self) -> Result<()> {
        // A send error means the service thread is gone; nothing to do about it here.
        let _ = self.tx.send(Msg::Refresh);
        Ok(())
    }
}

impl IMMNotificationClient_Impl for DeviceEvents_Impl {
    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _state: DEVICE_STATE) -> Result<()> {
        self.post()
    }

    fn OnDeviceAdded(&self, _id: &PCWSTR) -> Result<()> {
        self.post()
    }

    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> Result<()> {
        self.post()
    }

    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _id: &PCWSTR) -> Result<()> {
        if flow == eRender && role == eMultimedia {
            return self.post();
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
        // Fires constantly and carries nothing this service shows.
        Ok(())
    }
}

/// Volume and mute events of one attached endpoint.
#[implement(IAudioEndpointVolumeCallback)]
pub(super) struct VolumeEvents {
    pub(super) tx: Sender<Msg>,
    pub(super) generation: u64,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeEvents_Impl {
    fn OnNotify(&self, notify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
        // SAFETY: Core Audio passes a valid notification record for the duration of the call;
        // a null pointer is tolerated and ignored.
        if let Some(data) = unsafe { notify.as_ref() } {
            let _ = self.tx.send(Msg::Volume {
                generation: self.generation,
                volume: data.fMasterVolume,
                muted: data.bMuted.as_bool(),
            });
        }
        Ok(())
    }
}
