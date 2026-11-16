# plith-ipc

The named pipe protocol between the notch process and the UIAccess OSD helper.

Design: `docs/specs/2026-11-14-rust-rewrite-design.md`, section 7.

## Wire format

Little endian. Every message starts with `b'P' b'O' version kind`, then the payload.

| Kind | Name | Payload |
|---|---|---|
| 1 | Hello | `client_pid: u32` |
| 2 | Level | `meter u8, percent u8 (0..=100), muted u8 (0 or 1), label_len u8, label UTF-8 (max 64 bytes)` |
| 3 | Hide | none |
| 4 | Bye | none |

Meters: 0 volume, 1 brightness. A message is at most 128 bytes. The pipe is
`\\.\pipe\plith-osd-<session_id>`. A bad magic or a newer version is fatal
(`DecodeError::is_fatal`); an unknown kind is not.
