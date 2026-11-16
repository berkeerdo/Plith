# R4 renders 1: pages on the engine

Date: 2026-11-16. Offscreen WARP renders at scale 2 of the pages built by `plith-core` and drawn
by `plith-render`, each island flush under a black screen edge on a desk cell like the R3 mock
(`docs/design/r3/ray-paneli.html`). Data: `crates/plith-core/src/pages/fixtures.rs` (the mock's
data as facts). Source: `cargo run --release -p plith-render --example render_pages`, which
writes a new `%TEMP%\plith-renders-<seconds>-<pid>` folder each run.

| File | State |
|---|---|
| `01-audio-banana.png` | Audio, Voicemeeter: Windows and Discord to A1, microphone to B1, line in muted |
| `02-audio-pick.png` | Audio, microphone picked: dashed candidates on the rail below it |
| `03-audio-potato.png` | Audio, Potato 8 x 8 with a few routes (density check, open item 2) |
| `04-audio-no-voicemeeter.png` | Audio without Voicemeeter: Windows to the default G733, Edifier reachable |
| `05-media-playing.png` | Media, Midnight City at 1:42 of 4:03, A1 G733 lit (the route is read-only in R4) |
| `06-media-empty.png` | Media, nothing playing |
| `07-shelf-empty.png` | Shelf, empty |
| `08-system.png` | System, brightness 70, microphone open to B1 |
| `09-system-mic-muted.png` | System, microphone muted: route cut with `KAPALI` |
| `10-system-no-ddc.png` | System, monitor without DDC/CI |
| `11-pills.png` | Pill: playing 62, muted, idle |
| `12-theme-*.png` | The Banana audio page in Kağıt, York and Sodyum (Gece is `01`) |

Known differences from the mock are listed in the Task 15 report: bus values are gains in dB
shown as bare signed numbers (`-6`) where the mock shows 62 and 40, no reachable output branch
on the media page (read-only in R4), no clip notch, shorter source captions, no trace without Voicemeeter, no
monitor name when DDC/CI is off, and the mock's whole SVG sits 1 DIP right of the engine (its
1 px island border is outside the SVG origin).
