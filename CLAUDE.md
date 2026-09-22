# Plith

Modern Windows audio OSD with Voicemeeter-first design + integrated media controls. Replaces Windows' aging volume flyout with a Mica/rounded overlay that works over fullscreen games and shows now-playing media inline.

## Status

**The shelf is the notch's own page, released in 0.3.0.** Paging onto the shelf page
hands the frame to `Plith.DropCatcher`, which draws it at the notch's own 356 x 164 with the notch's
own rail, so a file is dragged straight out of the notch in one gesture. It has to work this way
rather than through a pane: Plith is high integrity in Release and `DoDragDrop` carries nothing from
there, and a press cannot be delegated between processes. Both measured.

What it costs, stated where the gain is: **ten files instead of fifteen**, two short lines of name
instead of two long ones, no Clear button (a context menu and `Ctrl+A` then `Delete` instead), and
a cross-process window swap on every page turn onto the shelf. Spec and plan:
`docs/superpowers/specs/2026-09-21-shelf-in-the-notch-design.md`.

**The handover is measured on hardware; the rest of that run is not.** See
`docs/SHELF-VERIFICATION.md` section 10, which names which half.

**0.3.1 is the current release, and everything in this file ships in it.** Phases 1 to 7 plus
brightness: Voicemeeter and Windows Core Audio, SMTC media, Settings with live theming,
Game mode (UIAccess-signed BandWindow), endpoint pinning, native flyout suppression, the
Ambient Notch with paged widgets, the shelf, and brightness.

Phase 6 slice 1 (the notch shape) shipped in 0.1.6; the notch widgets and the accent work
shipped in 0.1.8. 0.2.0 added the shelf as a pane, the media page rebuilt after Alcove with seek
and an output picker, the notch event rule, and brightness. 0.3.0 makes the shelf the notch's own
page: one surface instead of two, reached by a page turn instead of an expand. 0.3.1 is that
release plus the idle-cost fixes in `docs/PERF-VERIFICATION.md`.

**There is no auto-update.** Nothing in the repo checks for a new version, and updates are a
download from GitHub Releases. See `docs/ROADMAP.md` for the remaining Phase 4 item this
depends on (MSIX + a real signing certificate); the installer is self-signed today, so
SmartScreen warns on first run.

### What is deliberately not done

- Phase 4c-4: MSIX packaging and a SignPath OSS certificate.
- Phase 4g (optional): Sonar HTTP API deep integration.
- The ambient hover row that notch slice 3 made unreachable is still wired.
- `Palette.Dark` and `OsdPalette.Dark` each declare the accent green.
- The media page has no live waveform and no in-notch device list beyond the picker page;
  the spec records why for each.

### The lesson this repo keeps paying for

**A green build, green tests and green lints cannot see a wrong shape, a wrong layout, or a
window that is not on screen.** Every slice here has shipped green and then failed on the
first real use, and every defect found since has been found by looking at a render, driving
the real UI, or capturing the screen. Recent examples, each with a full ledger in the
verification documents: a layered window nothing had ever captured, a tile that answered a
pointer only where its icon painted, a contrast ratio that passed while drawing the bar
inverted, a shelf page laid out 7 DIP taller than the window holding it, and a tooltip that
outlived the tile it belonged to and read as a file coming back.

So: **measure it on a running build before saying it works, and say plainly which half you
measured.** Claims inherited from another surface's notes have been wrong four times.

### Where the measurements live

| Document | What it records |
|---|---|
| `docs/PHASE5-VERIFICATION.md` | CardHost, fullscreen-video auto-hide, accessibility |
| `docs/PHASE6-VERIFICATION.md` | The notch, its widgets, the media page, seek, the output picker, brightness |
| `docs/SHELF-VERIFICATION.md` | The shelf end to end, including the drop catcher and every instrument defect |
| `docs/PERF-VERIFICATION.md` | What the app costs at rest, on a launch and on a notch open, where inside the launch that time goes, and which of it has been fixed |
| `docs/ROADMAP.md` | Phase status, estimates beside actuals, and open questions |

### The instruments

Run these before claiming anything works. They are cheap and they have each caught defects
nothing else could see.

| Script | What it answers |
|---|---|
| `scripts/render-widgets.ps1` | What every surface looks like, in both themes and three accents, plus tree assertions no eye can make |
| `scripts/check-a11y.ps1` | Every interactive control has a name, and no view uses a glyph font |
| `scripts/check-contrast.ps1` | Text clears 4.5:1 and non-text 3:1, on both themes and every accent |
| `scripts/check-shared-xaml.ps1` | Shared XAML names no assembly (0.1.6 shipped a broken installer this way) |
| `scripts/drive-shelf-pair.ps1` | Drives the real Plith and catcher pair through the notch, on hardware |
| `scripts/drive-media-page.ps1` | Clicks the real notch and reads the live UI Automation tree |
| `scripts/measure-notch-open.ps1` | What one open of the notch costs, and whether the UI thread blocked during it |
| `scripts/measure-startup.ps1` | What one launch costs, split ten ways, its largest phase split ten ways again, and how long it blocks the UI thread |
| `scripts/build-release.ps1` | Builds and signs the installer. Needs admin and the signing cert |

A Debug build runs at **medium** integrity (`app.manifest` sets `uiAccess="false"`; only
Release swaps in the signed one), so UI Automation reads the OSD's tree and `SendInput` drives
it, over Remote Desktop included. The premise that none of this could be driven was false for
two whole phases.


## Stack
- **WPF + .NET 10 (LTS)** — proven topmost-over-fullscreen path via BandWindow + renamed `ApplicationFrameHost.exe` (borrowed from MIT-licensed VoicemeeterFancyOSD's Host/Bridge/Interop layer).
- Voicemeeter Remote API via `VoicemeeterRemote64.dll` P/Invoke.
- Windows Media Session via `Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager`.
- Mica/Acrylic via `DwmSetWindowAttribute` (Win11 21H2+).

## Read these in order
1. **PLAN.md** — full implementation plan (3 phases, file structure, conventions, references)
2. **NOTICE.md** — third-party credits (FancyOSD MIT)

## Quick reference

| Concept | Where |
|---|---|
| Architecture decisions | PLAN.md §3 (stack), §5 (phases) |
| File structure | PLAN.md §6 |
| Bootstrap commands | PLAN.md §7 |
| Reference projects to mine | PLAN.md §8 (FancyOSD, ModernFlyouts) |
| User's environment | PLAN.md §9 (G733, Voicemeeter Banana, Norton) |
| Open questions | PLAN.md §10 |

## Conventions (from user global CLAUDE.md)
- **All code/comments/commits in English.**
- Conventional Commits format.
- **Never** include "Co-Authored-By: Claude" or AI attribution in commits, code, or docs.
- Use Plan Mode before multi-file architectural changes.

## Next actions
1. `dotnet --list-sdks` — confirm .NET 10 SDK is installed (we have runtime; SDK may need `winget install Microsoft.DotNet.SDK.10`)
2. Bootstrap solution per PLAN.md §7
3. Begin Phase 1 (Voicemeeter-first MVP) per PLAN.md §5
