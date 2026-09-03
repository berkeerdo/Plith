# Plith

Modern Windows audio OSD with Voicemeeter-first design + integrated media controls. Replaces Windows' aging volume flyout with a Mica/rounded overlay that works over fullscreen games and shows now-playing media inline.

## Status
**Phases 1–4f shipped (0.1.5).** Voicemeeter + Windows Core Audio + SMTC media
integration, Settings UI with live theming, Game mode (UIAccess-signed BandWindow),
free-form hotkey capture, mixer-agnostic endpoint pinning (Sonar / Unify / Wave Link
channels), Win11-safe native flyout suppression, WH_KEYBOARD_LL hook.

**Phase 5 code-complete on `feature/phase-5-cardhost`, not yet merged.** The OSD now
renders through a `CardHost` service that owns card visibility and is the single
authority for when the OSD appears; today's OSD is an Audio card plus a Media card.
Adds fullscreen-video auto-hide (on by default, never fires during games) and an
accessibility pass. `OsdViewModel` is gone; `OsdOrchestrator` is a pure source driver.

**Phase 5 is partly verified.** Layout and accessibility were measured against a running
build; the remaining checks need a physical console session, because over RDP the OSD's
layered window cannot be captured by any means. Four real defects were found doing it —
all inside the "completed" accessibility pass, all fixed on the branch. A green build,
green tests and a green lint had missed every one, because nothing looked at the running
accessibility surface. The lint now covers three of the four.

Still open, in `docs/PHASE5-VERIFICATION.md`: §1.4, §2, §4.2-4.3, §5 and most of §6.
**Treat §2 (games must keep the OSD) as the merge blocker** — a false positive there
silently destroys the headline feature and no test in this repo can catch it.

Remaining from Phase 4: 4c-4 (MSIX + SignPath OSS cert) and optional 4g (Sonar HTTP
API deep integration).

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
