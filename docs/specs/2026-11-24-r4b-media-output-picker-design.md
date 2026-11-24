# R4b Media output picker: per-app output route

Date: 2026-11-24. Status: design, my call under Berke's "her işi yapsana" (2026-11-24); he can
veto any point. Facts: `research/2026-11-24-r4b-app-route.md` (EarTrumpet source, windows-rs
0.62.2). Look: R3 direction (`specs/2026-11-14-r3-visual-direction.md:47`, mock
`design/r3/ray-paneli.html:207-214`): "the output picker is a route `SPOTIFY > A1 · G733` with
reachable outputs as hollow stations".

## 1. Problem
The media page draws the route `SOURCE > OUTPUT` read-only, and OUTPUT is the Windows default
endpoint, not where the playing app really goes. Windows can route one app to another output
(Settings > Sound > Volume mixer), but only through the undocumented `IAudioPolicyConfigFactory`.

## 2. Goals, non-goals
Goals
- The route shows the playing app's real output: its persisted per-app endpoint if one is set,
  else the default endpoint.
- A click on the route's output opens pick mode in place on the media page: under the lit route,
  one hollow station per reachable output on a branch line (as in the mock), first `VARSAYILAN`
  (follow the Windows default), then the active render endpoints, the current one filled. Max 4
  stations; more outputs add a last word `DAHA FAZLA` that opens `ms-settings:apps-volume`.
- A station click routes the app (every matching pid), reads it back, leaves pick mode. Failure:
  pick mode stays, caption `ÇIKIŞ DEĞİŞMEDİ` (all eight languages), route unchanged.
- Leaving the island or a click on the route again leaves pick mode without a change (no Esc:
  the island has no keyboard, ADR 0011).
Non-goals
- Changing the Windows default output (WPF did that; the system page owns the default).
- Voicemeeter bus routing for the media app (the audio page owns Voicemeeter routes).
- Capture devices, the communications role, `ClearAllPersistedApplicationDefaultEndpoints`
  (a global reset; never called).

## 3. Fail closed (the interface has no contract)
- Step 0 is a probe example `crates/plith-audio/examples/probe_app_route.rs`: it opens its own
  silent render stream, finds its own session, calls Get on its own pid, Set to another active
  endpoint, Get again (must read back the same wrapped id), Set null (clear), Get again (empty).
  It only ever touches its own process. Its output on build 26300 is recorded before any UI work.
- At service start the factory is activated once (IID `ab3d4648-...` for build >= 21390, else
  `2a59116d-...`). Activation failure, or a Get that is not S_OK with a parsable id or empty,
  marks the feature unavailable for the run: the route stays read-only (today's behaviour),
  one log line with build and HRESULT.
- Every Set is followed by a Get; a mismatch is a failure. One failed Set disables the picker for
  the run (no retry storms).
- Only ids from the active endpoint list, wrapped with the fixed render prefix and suffix.
- Roles: eMultimedia and eConsole, never eCommunications (as EarTrumpet and WPF).

## 4. Shape
- `plith-core`: id wrap and unwrap, the SMTC AUMID to process match rules (packaged AUMID, exe
  stem, aliases for browsers), station choice and order, pick mode state and hit targets on the
  media page, all with unit tests.
- `plith-audio`: the factory interface (IInspectable plus 19 placeholder slots, then Set, Get,
  ClearAll: count measured by the probe), session enumeration over every active render endpoint
  (`IAudioSessionManager2`, `IAudioSessionControl2::GetProcessId`), runs on the endpoints
  service thread like the other writes. New features: `Win32_System_WinRT`, and only what the
  match needs.
- `plith`: the media page input routes a station click to the service; the snapshot carries the
  app's route and the picker's availability.
- No match (player closed or no audio session yet): pick mode shows only the caption
  `ÇALAN UYGULAMA BULUNAMADI` and no stations.

## 5. Proof
- Probe output on this PC (any session, RDP is fine: it is not motion).
- Unit tests for every pure rule; five checks green.
- On screen with Berke: route Spotify to a second output while playing, hear it move, back to
  `VARSAYILAN`. Motion of pick mode measured on the console only.
