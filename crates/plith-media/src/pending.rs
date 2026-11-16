//! Which reads a batch of worker messages calls for.

/// Which reads a batch of messages calls for.
#[derive(Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags that mirror SMTC"
)]
pub(crate) struct Pending {
    pub(crate) session: bool,
    pub(crate) properties: bool,
    pub(crate) playback: bool,
    pub(crate) timeline: bool,
    pub(crate) stop: bool,
}
