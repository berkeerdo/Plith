//! Built-in panels through WMI (`root\wmi`), plus the change listener. `cfg(windows)` only.
//!
//! The level lives in `WmiMonitorBrightness`, the write in `WmiMonitorBrightnessMethods`
//! (`WmiSetBrightness`); both are matched by `InstanceName`. A desktop has no instances and
//! that is a normal empty state, not a failure. Every call connects afresh and every wait is
//! capped at about a second, so a wedged provider cannot hold the worker.

use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
    RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::Variant::{
    VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BSTR, VT_I2, VT_I4, VT_I8, VT_UI1,
    VariantClear,
};
use windows::Win32::System::Wmi::{
    IWbemClassObject, IWbemContext, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WbemLocator,
};
use windows::core::{BSTR, PCWSTR, w};

use super::BrightnessDevice;
use super::wql::{NAMESPACE, methods_query, panel_id, read_query};

/// Longest wait on one WMI enumerator step.
const QUERY_WAIT_MS: i32 = 1000;
/// How often the listener looks at its stop flag.
const EVENT_POLL_MS: i32 = 500;
/// Seconds the panel driver may take for `WmiSetBrightness`.
const SET_TIMEOUT_SECONDS: i32 = 1;

/// An owned `VARIANT`, cleared on drop.
struct Variant(VARIANT);

impl Variant {
    fn build(vt: VARENUM, data: VARIANT_0_0_0) -> Self {
        Self(VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                    vt,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: data,
                }),
            },
        })
    }

    fn i4(value: i32) -> Self {
        Self::build(VT_I4, VARIANT_0_0_0 { lVal: value })
    }

    fn u1(value: u8) -> Self {
        Self::build(VT_UI1, VARIANT_0_0_0 { bVal: value })
    }

    fn as_i64(&self) -> Option<i64> {
        // SAFETY: the active union member is the one `vt` names.
        unsafe {
            let inner = &self.0.Anonymous.Anonymous;
            match inner.vt {
                VT_UI1 => Some(i64::from(inner.Anonymous.bVal)),
                VT_I2 => Some(i64::from(inner.Anonymous.iVal)),
                VT_I4 => Some(i64::from(inner.Anonymous.lVal)),
                VT_I8 => Some(inner.Anonymous.llVal),
                _ => None,
            }
        }
    }

    fn as_string(&self) -> Option<String> {
        // SAFETY: the active union member is the one `vt` names.
        unsafe {
            let inner = &self.0.Anonymous.Anonymous;
            (inner.vt == VT_BSTR).then(|| inner.Anonymous.bstrVal.to_string())
        }
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        // SAFETY: the variant is initialized (zeroed or built above) and cleared once.
        let _ = unsafe { VariantClear(&raw mut self.0) };
    }
}

fn property(object: &IWbemClassObject, name: PCWSTR) -> Option<Variant> {
    let mut value = Variant(VARIANT::default());
    // SAFETY: `value.0` is a valid, empty VARIANT the call fills.
    unsafe { object.Get(name, 0, &raw mut value.0, None, None) }.ok()?;
    Some(value)
}

/// Connects to `root\wmi` on this thread, which must have COM initialized.
fn connect() -> windows::core::Result<IWbemServices> {
    // SAFETY: plain COM activation and call on an initialized thread.
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let services = locator.ConnectServer(
            &BSTR::from(NAMESPACE),
            &BSTR::new(),
            &BSTR::new(),
            &BSTR::new(),
            0,
            &BSTR::new(),
            None::<&IWbemContext>,
        )?;
        CoSetProxyBlanket(
            &services,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
        Ok(services)
    }
}

const QUERY_FLAGS: WBEM_GENERIC_FLAG_TYPE =
    WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0);

/// Runs a WQL select and returns its rows.
fn query(services: &IWbemServices, wql: &str) -> windows::core::Result<Vec<IWbemClassObject>> {
    // SAFETY: plain COM calls; the row slot and the count are live locals.
    unsafe {
        let rows = services.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(wql),
            QUERY_FLAGS,
            None::<&IWbemContext>,
        )?;
        let mut found = Vec::new();
        loop {
            let mut slot = [None];
            let mut returned = 0u32;
            let status = rows.Next(QUERY_WAIT_MS, &mut slot, &raw mut returned);
            if status.is_err() || returned == 0 {
                break;
            }
            found.extend(slot[0].take());
        }
        Ok(found)
    }
}

/// One built-in panel, found by `InstanceName`.
struct WmiPanel {
    instance: String,
    id: String,
}

impl BrightnessDevice for WmiPanel {
    fn id(&self) -> &str {
        &self.id
    }

    fn read(&mut self) -> Option<u8> {
        let services = connect().ok()?;
        let rows = query(&services, &read_query(&self.instance)).ok()?;
        let level = property(rows.first()?, w!("CurrentBrightness"))?.as_i64()?;
        u8::try_from(level.clamp(0, 100)).ok()
    }

    fn write(&mut self, percent: u8) -> bool {
        set_brightness(&self.instance, percent).is_ok()
    }
}

/// Calls `WmiSetBrightness` on the panel's method object.
fn set_brightness(instance: &str, percent: u8) -> windows::core::Result<()> {
    let services = connect()?;
    let rows = query(&services, &methods_query(instance))?;
    let path = rows
        .first()
        .and_then(|row| property(row, w!("__PATH")))
        .and_then(|value| value.as_string())
        .ok_or_else(|| windows::core::Error::from(E_FAIL))?;
    // SAFETY: plain COM calls; the out slots are live locals.
    unsafe {
        let mut class = None;
        services.GetObject(
            &BSTR::from("WmiMonitorBrightnessMethods"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None::<&IWbemContext>,
            Some(&raw mut class),
            None,
        )?;
        let class = class.ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let mut signature: Option<IWbemClassObject> = None;
        class.GetMethod(
            w!("WmiSetBrightness"),
            0,
            &raw mut signature,
            std::ptr::null_mut(),
        )?;
        let params = signature
            .ok_or_else(|| windows::core::Error::from(E_FAIL))?
            .SpawnInstance(0)?;
        params.Put(w!("Timeout"), 0, &Variant::i4(SET_TIMEOUT_SECONDS).0, 0)?;
        params.Put(w!("Brightness"), 0, &Variant::u1(percent).0, 0)?;
        services.ExecMethod(
            &BSTR::from(path),
            &BSTR::from("WmiSetBrightness"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None::<&IWbemContext>,
            &params,
            None,
            None,
        )
    }
}

/// Every built-in panel this machine reports, or nothing (the normal answer on a desktop).
pub(super) fn enumerate() -> Vec<Box<dyn BrightnessDevice>> {
    let Ok(services) = connect() else {
        return Vec::new();
    };
    let rows =
        query(&services, "SELECT InstanceName FROM WmiMonitorBrightness").unwrap_or_default();
    rows.iter()
        .filter_map(|row| property(row, w!("InstanceName"))?.as_string())
        .filter(|name| !name.is_empty())
        .map(|instance| {
            Box::new(WmiPanel {
                id: panel_id(&instance),
                instance,
            }) as Box<dyn BrightnessDevice>
        })
        .collect()
}

/// Blocks on `WmiMonitorBrightnessEvent` (internal panels only; an external monitor never
/// announces anything) and calls `on_event(instance, percent)` for each, until `stop` is set.
/// The calling thread must have COM initialized. Returns early when WMI cannot be reached.
pub(super) fn listen(stop: &AtomicBool, mut on_event: impl FnMut(String, u8)) {
    let Ok(services) = connect() else {
        return;
    };
    // SAFETY: plain COM call.
    let Ok(events) = (unsafe {
        services.ExecNotificationQuery(
            &BSTR::from("WQL"),
            &BSTR::from("SELECT * FROM WmiMonitorBrightnessEvent"),
            QUERY_FLAGS,
            None::<&IWbemContext>,
        )
    }) else {
        return;
    };
    while !stop.load(Ordering::Acquire) {
        let mut slot = [None];
        let mut returned = 0u32;
        // SAFETY: the slot and the count are live locals.
        let status = unsafe { events.Next(EVENT_POLL_MS, &mut slot, &raw mut returned) };
        if status.is_err() {
            return;
        }
        let Some(event) = slot[0].take().filter(|_| returned > 0) else {
            continue;
        };
        let instance = property(&event, w!("InstanceName")).and_then(|v| v.as_string());
        let percent = property(&event, w!("Brightness")).and_then(|v| v.as_i64());
        if let (Some(instance), Some(percent)) = (instance, percent) {
            on_event(instance, u8::try_from(percent.clamp(0, 100)).unwrap_or(0));
        }
    }
}
