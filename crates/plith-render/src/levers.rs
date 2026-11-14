//! Applies the load levers to the OS. A refusal is an outcome, never an error: the caller logs
//! it with `outcome_line` and keeps running with the default.

use plith_core::levers::{LeverOutcome, Levers, MmcssPriority, SchedClass};
use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_SCHEDULINGPRIORITYCLASS, D3DKMT_SCHEDULINGPRIORITYCLASS_ABOVE_NORMAL,
    D3DKMT_SCHEDULINGPRIORITYCLASS_HIGH, D3DKMTSetProcessSchedulingPriorityClass,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGIDevice1};
use windows::Win32::System::Threading::{
    AVRT_PRIORITY, AVRT_PRIORITY_CRITICAL, AVRT_PRIORITY_HIGH, AvSetMmThreadPriority,
    GetCurrentProcess,
};
use windows::core::{Interface, Result};

/// A 32-bit status code as hex text, for example `0x80070005`.
#[must_use]
pub fn hex(code: i32) -> String {
    format!("0x{:08X}", code.cast_unsigned())
}

fn outcome(name: &'static str, setting: String, result: Result<()>) -> LeverOutcome {
    LeverOutcome {
        name,
        setting,
        result: result.map_err(|error| hex(error.code().0)),
    }
}

/// `IDXGIDevice::SetGPUThreadPriority`; `None` at the default 0 (no call is made).
#[must_use]
pub fn apply_gpu_priority(device: &IDXGIDevice, priority: i8) -> Option<LeverOutcome> {
    if priority == 0 {
        return None;
    }
    // SAFETY: a plain COM call on a live device; the priority is within -7..=7.
    let result = unsafe { device.SetGPUThreadPriority(i32::from(priority)) };
    Some(outcome("gpu-priority", priority.to_string(), result))
}

/// `IDXGIDevice1::SetMaximumFrameLatency`. The outcome is always returned for the log; a failing
/// call is fatal only for the default (`Levers::default().latency`), any other value is just
/// reported as refused.
///
/// # Errors
/// Returns the failing call when `latency` is the default.
pub fn apply_latency(device: &IDXGIDevice, latency: u8) -> Result<Option<LeverOutcome>> {
    let device1 = device.cast::<IDXGIDevice1>()?;
    // SAFETY: a plain COM call on a live device.
    let result = unsafe { device1.SetMaximumFrameLatency(u32::from(latency)) };
    if latency == Levers::default().latency
        && let Err(error) = &result
    {
        return Err(error.clone());
    }
    Ok(Some(outcome("latency", latency.to_string(), result)))
}

/// `D3DKMTSetProcessSchedulingPriorityClass` on this process; `None` for `Normal`. It is
/// process-wide: call it once. The process priority class is never touched.
#[must_use]
pub fn apply_sched_class(class: SchedClass) -> Option<LeverOutcome> {
    let value: D3DKMT_SCHEDULINGPRIORITYCLASS = match class {
        SchedClass::Normal => return None,
        SchedClass::AboveNormal => D3DKMT_SCHEDULINGPRIORITYCLASS_ABOVE_NORMAL,
        SchedClass::High => D3DKMT_SCHEDULINGPRIORITYCLASS_HIGH,
    };
    // SAFETY: the pseudo handle of the current process is always valid and needs no close.
    let status = unsafe { D3DKMTSetProcessSchedulingPriorityClass(GetCurrentProcess(), value) };
    let result = if status.0 >= 0 {
        Ok(())
    } else {
        Err(hex(status.0))
    };
    Some(LeverOutcome {
        name: "sched-class",
        setting: class.as_str().to_string(),
        result,
    })
}

/// `AvSetMmThreadPriority` for the MMCSS task `task`; `None` for `Normal`.
#[must_use]
pub fn apply_mmcss(task: HANDLE, priority: MmcssPriority) -> Option<LeverOutcome> {
    let value: AVRT_PRIORITY = match priority {
        MmcssPriority::Normal => return None,
        MmcssPriority::High => AVRT_PRIORITY_HIGH,
        MmcssPriority::Critical => AVRT_PRIORITY_CRITICAL,
    };
    // SAFETY: `task` came from `AvSetMmThreadCharacteristicsW` on this thread and is not reverted.
    let result = unsafe { AvSetMmThreadPriority(task, value) };
    Some(outcome("mmcss", priority.as_str().to_string(), result))
}

#[cfg(test)]
mod tests;
