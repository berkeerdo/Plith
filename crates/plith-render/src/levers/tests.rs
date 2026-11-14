use super::*;

#[test]
fn hex_formats_an_hresult_as_eight_digits() {
    assert_eq!(hex(0x8007_0005_u32.cast_signed()), "0x80070005");
    assert_eq!(hex(0), "0x00000000");
}

#[test]
fn default_levers_make_no_os_call() {
    assert!(apply_sched_class(SchedClass::Normal).is_none());
    assert!(apply_mmcss(HANDLE::default(), MmcssPriority::Normal).is_none());
}
