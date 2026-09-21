# Plith

Modern Windows audio OSD with Voicemeeter-first design + integrated media controls. Replaces Windows' aging volume flyout with a Mica/rounded overlay that works over fullscreen games and shows now-playing media inline.

## Status

**0.2.0 is the current release. Everything below is in `main`.** Phases 1 to 7 plus
brightness: Voicemeeter and Windows Core Audio, SMTC media, Settings with live theming,
Game mode (UIAccess-signed BandWindow), endpoint pinning, native flyout suppression, the
Ambient Notch with paged widgets, the shelf, and brightness.

Phase 6 slice 1 (the notch shape) shipped in 0.1.6; the notch widgets and the accent work
shipped in 0.1.8. 0.2.0 adds the shelf, the media page rebuilt after Alcove with seek and an
output picker, the notch event rule, and brightness.

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
