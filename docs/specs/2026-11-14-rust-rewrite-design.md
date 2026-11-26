# Plith in Rust: rewrite design

Date: 2026-11-14. Status: approved by Berke 2026-11-14.
Inputs: `docs/research/2026-11-14-design-review.md`, `docs/research/2026-11-14-oyenza-engine.md`.

## 1. Intent

Berke installed Oyenza Studio and found Plith (WPF 0.3.1) much less fluid; the shelf in particular
feels wrong. WPF cannot produce continuous, interruptible spring morphs, and the shelf needs a
second process only because the whole app runs at high integrity. The decision (2026-11-14) is a
complete redesign and a from-scratch Rust rewrite, not a port.

Positioning: **the notch for people who care about sound and play games.** Few pages, each one
excellent. The WPF app stays the working reference and keeps shipping until the Rust build
reaches parity (section 9), then it is deleted from the repo in one commit.

## 2. Goals

- G1 Motion at Oyenza's bar or better, measured on this PC (RTX 5080, 240 Hz, console session):
  every motion at 238+ fps with 0 to 1 hitches, draw under 1 ms per frame at p99. Durations in
  Oyenza's range (expand ~730 ms, collapse ~660 ms, page ~450 ms) unless a design reason says
  otherwise.
- G2 Zero cost at rest: no frames, no periodic wake-ups of the render thread.
- G3 Pages: audio (Voicemeeter buses and strips with meters and mute, Windows endpoints, endpoint
  pinning), media (SMTC with seek and output picker), shelf (in-process drag in and out), system
  (brightness, mic mute). A visible tab bar; pages sized to their content; the island morphs
  between sizes.
- G4 The volume, brightness and mute OSD shows over exclusive fullscreen games.
- G5 Native Windows volume and brightness flyouts are suppressed while Plith runs.

## 3. Non-goals and anti-goals

Non-goals for the parity release: macOS, a glass or desktop-duplication theme, add-ons or
plugins, auto-update (still blocked on the signing certificate, ROADMAP), widgets beyond the four
pages. Dropped from WPF by Berke's call (2026-11-14): the weather page and the accent picker;
one accent remains (lime, from Loft). No calendar or timers.

Anti-goals, failures even if everything "works":
- A motion that drops frames at 240 Hz or a UI-thread stall that stalls a morph.
- A second process for the shelf, or a window swap on any page turn.
- Feature soup: a tab bar that grows with things nobody asked for.
- Any claim about look or motion that was not rendered or measured on a running build (the
  lesson in `docs/wpf/CLAUDE-wpf.md`).

## 4. Constraints

- Windows 11, x64. Rust stable (1.98 today), edition 2024, Cargo resolver 3.
- Praxvon structure limits (500 source, 150 entry, 800 test lines; 80-line functions), clippy
  lints from `praxvon-structure/references/rust-tauri.md`, `scripts/structure-check.mjs` in CI.
- No script may move the mouse on this PC. Motions are triggered by a demo driver (a named pipe
  command or `--demo` flag), never by synthesized input.
- Motion is measured only in a console session; RDP turns client-area animation off.
- Heavy builds go through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.

## 5. Approaches considered

| | Approach | Trade-off | Verdict |
|---|---|---|---|
| A | Win32 + D3D11 + Direct2D + DirectComposition, own render thread, own tiny UI layer | Most code to write; full control of the frame, the only path to beat G1 | **Chosen** |
| B | Tauri / WebView2 overlay | Fast UI work; a browser compositor between us and vblank, a 100+ MB process, no exclusive-fullscreen path | Rejected |
| C | An existing Rust GUI (egui, iced, Slint) on wgpu | Faster start; none controls a fixed oversized per-pixel-alpha topmost surface, vblank stamping or click-through the way G1 needs | Rejected |

Repo: **same repo** (`berkeerdo/Plith`), Rust workspace at the root next to the frozen WPF tree.
Keeps the GitHub Releases page users download from, the issue history and this project's memory.
WPF docs stay where they are and are marked frozen in `docs/README.md` (ADR 0001).

## 6. Architecture

### Processes (ADR 0002)
- `plith.exe`, **medium integrity**: notch, all pages, services, tray, settings. OLE drag and drop
  works in-process, so the shelf is one surface.
- `plith-osd.exe`, **UIAccess, signed, installed under Program Files**: draws only the volume,
  brightness and mute OSD over exclusive fullscreen and over elevated windows, using the band
  window technique measured in WPF Game mode. Receives state from `plith.exe` over a named pipe.
- Consequence, accepted: the notch itself hides in exclusive fullscreen and may not take clicks
  while an elevated window is in front. Borderless games keep the notch.

### Threads inside `plith.exe`
- **UI thread** (STA): message loop, input, hover state machine, OLE drag and drop, service events.
  Never draws.
- **Render thread**: owns the D3D11 device, Direct2D device context, DirectWrite and the
  DirectComposition tree. Runs the frame clock, steps every spring, draws, presents. Reads the
  latest scene state the UI thread published; publishes the exact island shape for hit testing.
- **Service threads**: Voicemeeter polling, Core Audio callbacks, SMTC, brightness. They publish
  state to the UI thread; nothing in them touches rendering.

### Frame clock
- Waits on the compositor or display vblank (candidates to measure: `DCompositionWaitForCompositorClock`,
  a frame-latency waitable swap chain, `IDXGIOutput::WaitForVBlank`; the builder picks by
  measurement).
- Phase-locked timeline; animation time is stamped to the vblank the frame will appear on;
  every step is a whole number of refresh periods; first step after rest is exactly one period.
- Parks completely at rest. Ambient work (meters, seek bar) runs at a divisor of the nominal
  refresh rate and only while visible.

### Surface and shape
- One fixed, oversized surface per monitor; the HWND never moves or resizes during a morph.
- Island width, height and corner radius are independent springs. Grow and shrink tunings per
  axis, chosen at retarget time from that axis's direction; velocity is kept on every retarget;
  a shrinking axis lands on its floor and never squashes. Oyenza's numbers are the starting
  baseline (research doc, section 4).
- Page turns are springs too (position and opacity), so an interrupted turn keeps its velocity.
- Content that moves during a morph is drawn into an offscreen layer and composited with group
  opacity, so text is rasterized once.
- Hit testing follows the drawn shape every frame (Oyenza: 30 Hz). The mechanism is the builder's
  call by spike (per-frame mask window, window region, or input-transparent toggling); the
  acceptance is that clicks outside the shape reach the window beneath with mouse, pen and touch,
  and clicks inside are never lost, within one frame of the shape.

### Measurement from day one
- `%LOCALAPPDATA%\Plith\plith.log` gets one `motion` line per motion: kind, duration, frames, fps
  at the refresh rate, hitches (gap over 1.5 periods), worst gap, draw and present ms at p50 and
  p99. Same format as Oyenza's so the two can be put side by side.
- An idle check: 60 s at rest must show zero render-thread wake-ups.

## 7. Workspace

```
Cargo.toml                  [workspace], resolver 3, shared lints and dependency versions
crates/plith-core/          pure logic, no Windows: springs, timeline, motion stats, island
                            state machine, geometry, page layout; unit tested
crates/plith-render/        D3D11, Direct2D, DirectWrite, DirectComposition, render thread,
                            frame clock, layers, text
crates/plith-shell/         notch window, hit testing, input, hover, topmost, fullscreen
                            detection, OLE drag and drop, tray
crates/plith-audio/         Voicemeeter Remote API, Core Audio endpoints, pinning, mic mute
crates/plith-media/         SMTC sessions, seek, output picker data
crates/plith-system/        brightness, native flyout suppression
crates/plith-ipc/           notch to OSD pipe protocol
crates/plith/               the app binary: composes everything, thin main.rs
crates/plith-osd/           the UIAccess helper binary
crates/plith-host/          Windows helpers shared by the binaries: log, language, Run key,
                            WinHTTP GET, signature check (added 2026-11-26 from the R7 crates)
crates/plith-setup/         the installer binary (added 2026-11-26 from the R7 crates)
```
Dependency direction: `plith` depends on everything; `plith-render` and `plith-shell` depend on
`plith-core`; the three service crates depend on nothing of ours. Windows bindings: the
`windows` crate, one pinned version in `[workspace.dependencies]`.
Added 2026-11-26 from the R7 crates: `plith-host` depends on `plith-core`; `plith-weather` may
also use `plith-host`; `plith-setup` may use `plith-core`, `plith-host`, `plith-ipc`,
`plith-media`, `plith-render` and `plith-shell`; all three may use `windows`. Crates other than
`plith` may not depend on each other beyond these rules (`scripts/check-deps.mjs` enforces them).
Exception: `plith-render` may use `plith-media` as a dev-dependency only (tests and examples
decode real art); a normal dependency stays forbidden.

## 8. Visual design

Not decided here. It comes from a `praxvon-direction` session with Berke (resting pill, tab bar,
the four pages, type system with tabular numerals, one icon set), then `impeccable`. Starting
point: the Loft palette from branch `feature/notch-in-loft` (zinc floor, lime only as a marker,
praxvon-ui ADR 0014). Slice 1 needs no design: a plain dark island.

## 9. Order of work (ROADMAP carries the status)

1. R0 Setup: docs, workspace, lints, CI (master).
2. R1 Engine slice: empty island, morph, frame clock, hit testing, motion log, demo driver.
3. R2 Services, headless: audio, media, system crates, each with a probe example run on hardware.
4. R3 Visual direction with Berke.
5. R4 Pages on the engine: tab bar, audio, media, system.
6. R5 Shelf: OLE drag in and out.
7. R6 OSD helper: UIAccess, band window, pipe, signing.
8. R7 Settings, tray, installer; parity check against WPF 0.3.1; WPF removed.

R1 and R2 are independent (disjoint crates, no shared code) and each spans several sessions, so
they run as parallel lanes; R3 runs in the master with Berke meanwhile.

## 10. Calls made after the builder check (master's, correctable)

- Topmost: `plith.exe` keeps itself above borderless games with an event-driven topmost fight
  (foreground change, then a few short retries), as Oyenza does. R1 acceptance includes it
  holding over two borderless games on this PC. If it does not hold, ADR 0002 is revisited.
- OSD rendering: `plith-osd` reuses `plith-render`. R6 opens with a spike proving a DComp swap
  chain shows in a band window over exclusive fullscreen; if not, the helper draws Direct2D into
  a layered window instead (software target, small surface). No second renderer otherwise.
- Signing: the existing self-signed certificate and the install-under-Program-Files flow of
  `scripts/build-release.ps1` carry over to R6; a real certificate stays a ROADMAP item.
- Service contract: each service crate exposes its own typed snapshot plus a change channel, with
  no shared types from `plith-core`; `crates/plith` maps them to page state. The master writes
  the workspace `Cargo.toml` and lint config in R0 before any lane opens.
- Hardware: the notch lives on one chosen monitor (primary by default), per-monitor DPI v2, the
  frame clock follows that monitor's refresh rate. WARP is the only fallback (same Direct2D
  path); no software renderer. Over RDP the clock falls back to a timer and nothing about motion
  is measured there.
- Settings: TOML at `%APPDATA%\Plith\settings.toml`. Endpoint pins are migrated from the WPF
  settings in R7.
- Voicemeeter: standard, Banana and Potato through the same Remote API; strip and bus counts come
  from the reported type. Layout is designed and tested on Banana (Berke's setup).

## 11. Left to the builder

Frame clock source, hit-testing mechanism, swap chain buffer count and latency setting, scene
handoff between threads (snapshot swap or channel), the UI layer's shape (retained or immediate),
service polling rates. Each is chosen by measurement and recorded as an ADR.
