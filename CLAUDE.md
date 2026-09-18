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

**§2 (games must keep the OSD) passed on real hardware and is no longer a release gate.**
Run with `HideDuringFullscreenVideo` enabled, a game covering the monitor, and Spotify
actually playing — the combination that makes the check meaningful rather than vacuous:
the OSD appeared on every volume key, and not one `Suppression -> True` was logged. The
predicate behind it is unit-tested (`FullscreenVideoDetector.AumidMatchesProcess`), and the
measured AUMID makes a false positive unreachable on this configuration anyway: Spotify's
Store build reduces to the stem `SpotifyAB`, which no game process name can equal.

Also measured with a game running: it reported `QUNS_BUSY`, not
`QUNS_RUNNING_D3D_FULL_SCREEN`, confirming that Fullscreen Optimizations put it on the
composited path and that the D3D veto is dead code for such titles.

Remaining from Phase 4: 4c-4 (MSIX + SignPath OSS cert) and optional 4g (Sonar HTTP
API deep integration).

**Phase 7 slice 1 (the Shelf) code-complete on `feature/shelf-drop-catcher`, not yet merged.**
A file dropped on the notch now lands on a shelf page. The roadmap called this blocked, and it
was: UIAccess puts Plith at High integrity and UIPI refuses Explorer's cross-integrity drag, by
both the OLE and the `WM_DROPFILES` route. It works through a second process — `Plith.DropCatcher`,
Medium integrity, launched via Explorer so it does not inherit Plith's token — which takes the
notch's place for the duration of a drag and hands the paths back over a named pipe with an
explicit ACL.

**Verified on hardware, unusually for this repo.** The drop, the round trip, the shelf page and
persistence were all driven on a running build; three defects were found that way with a green
build behind them (a window WPF did not consider shown, a layered window nothing could capture,
and one approach threshold used for two different questions). Two more came from pointing
`check-contrast.ps1` at colours set in code, which it could not see before and now can.

**Dragging back OUT is blocked too, measured by controlled comparison:** the same binary returns
`None` from High with nothing copied and `Copy, Move` from Medium with the file landing. So the
catcher must become the drag source as well — a different design, deliberately unplanned. That
plus the Alcove/Dropover-style shelf actions the user asked for (clear, remove, open in Explorer,
stacking, real shell icons) is the next slice and needs its own plan.

Full ledger: `docs/superpowers/plans/2026-09-17-shelf-drop-catcher.md` (Tasks 1-6 done, Task 7
measured), and `docs/ROADMAP.md` Phase 7 for every measurement behind it.

**Phase 6 slice 1 code-complete on `feature/phase-6-notch-shell`, not yet merged.**
Adds presentation modes behind a new `IOsdPresentation` seam: Classic OSD (unchanged
behaviour, moved behind the seam) and Ambient Notch — a 190 DIP wide shape of
configurable height (2–24 DIP) resting at top-center that **grows** into the full
panel on event/hover and shrinks back, and that falls back to Classic wholesale
while a window covers the monitor. Settings gained a presentation picker and a
resting-height slider. Default stays Classic OSD for every install. Full Notch is
deferred — its always-on cards (System Controls' mic status, a Clock card) don't
exist yet. Multi-monitor: the notch pins to the saved monitor device name, falling
back to primary — this closes ROADMAP §10's open question.

**The first design was run and rejected.** It parked a full-width strip over the
card and translated the card down from behind it. On screen that reads as a drawer
opening, not a notch, and the strip's width tracked the window so a media session
appearing moved the anchor. It is now one surface whose width, height, corner radius,
shadow and content opacity all derive from a single expansion value, with the content
fading in only after the shape has mostly settled. Spec §2 records why.

**Phase 6 slice 2 (notch home view) code-complete on the same branch, not yet
merged.** Hovering the resting notch now opens an ambient row above the OSD's other
cards — a clock, current weather, and battery — with each column collapsing
independently (no battery column on a desktop; weather disappears with no location,
no network, or a stale reading). A volume or media event never opens the row; only a
deliberate hover does, and Classic OSD never opens it at all. Weather is Open-Meteo
(keyless), location priority is a typed city, then Windows Location, then IP
geolocation, refreshed every 15 minutes; Windows Location is asked on every refresh
rather than cached, since it does not re-prompt after the first answer. Settings
gained "Show weather" and "Weather location", hidden while Classic is selected.
(The "Show ambient info on hover" toggle this slice added is gone — slice 3 made hover a
peek and a click the way in, so it described a behaviour that no longer happens.)

**Phase 6 slice 3 (notch widgets) code-complete on the same branch, not yet merged.**
The open notch is now a fixed 356×116 frame with paged widgets — clock, weather, media,
audio — reached by a two-finger swipe, a tilt wheel, `Shift`+wheel or a click on the page
dots. An event no longer opens that frame: a volume key or a track change gets its own
short HUD shape instead, because an answer to something you did must not look like a place
you went. Plith also gained its **first audio write path** — the audio widget's level is
draggable, on Voicemeeter and on a Windows endpoint.

**The Classic OSD and Settings redesign is code-complete too, through Task 7 of its plan.**
Every icon in the product is now drawn geometry rather than Segoe MDL2, and the
accessibility lint fails the build on any that are not — in code-behind as well as XAML,
which was the rule's own blind spot on the day it was written. The classic card is sized by
its fullest row (300 DIP, 224 compact), titles scroll rather than ellipse, and Settings has
a grouped left rail and a preview that finally knows the notch exists.

**None of it is verified on a running build.** Build, 344 tests and the lint are green, and
none of that reaches any of it: the suite is not STA, and the OSD renders in a layered
window nothing can capture over RDP. Slice 2 shipped equally green and then crashed on the
first hover. Full ledgers in `docs/PHASE6-VERIFICATION.md` §15 and §16; the highest-risk
items are the three provisional paging constants, which were chosen without hardware and can
only be corrected from the log line each commit writes.

Two things are deliberately left undone: the ambient row that slice 3 made unreachable is
still wired (its removal is gated on slice 3 being driven on hardware first), and
`Palette.Dark` and `OsdPalette.Dark` still each declare the accent green.

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
