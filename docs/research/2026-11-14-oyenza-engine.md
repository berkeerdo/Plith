# Oyenza Studio engine and design benchmark (2026-11-14)

Source: github.com/lucacadalora/oyenza-studio, shallow clone at `scratchpad/oyenza-src` (MIT, never built or run). Version in manifest: 0.2.30. Paths below are relative to `crates/`. All numbers are read from code, not measured on a running binary. Nothing here was executed, so "smooth" is the author's claim backed by his own motion logs, not our measurement.

## 0. Verdict in five lines

- Oyenza is a **CPU-side Direct2D drawing loop, vblank-locked, with spring physics**, not a compositor-driven animation. Every animated frame is redrawn on the UI thread and presented via a flip swap chain.
- Its smoothness comes from three disciplined things: a phase-locked vblank timeline, animation time stamped to the *vblank the frame will appear on*, and fixed-size window so nothing is ever resized or moved during a morph.
- Its weaknesses: two windows per island, a 30 Hz click-through mask that trails the shape, no DirectComposition animation (so UI-thread stalls stall motion), no game-overlay path that survives exclusive fullscreen, and a very large feature surface (135k lines) that dilutes the core.
- Design is deliberately Apple-like: pure black island, SF-style type scale, system accent colours, one white-alpha ink ladder.
- The spring numbers are in section 4 and are directly reusable as a baseline (grow zeta 0.73, shrink zeta 0.9).

## 1. Frame clock

Files: `poni/src/window.rs`, `poni-core/src/frametime.rs`, `poni/src/app.rs`.

- **Thread**: one dedicated thread `poni-frame-clock` (window.rs:496-499, body 566-667). It posts `WM_APP_FRAME` to the island HWND (window.rs:663). The UI thread does all drawing.
- **Wait source** (window.rs:380-424), three tiers:
  1. `IDXGIOutput::WaitForVBlank` on the output matching the island's `HMONITOR` (window.rs:362-378, 411). This is the real display vblank at any rate.
  2. `DwmFlush` if no DXGI output (remote sessions) (window.rs:412).
  3. High-resolution sleep timer at one period, with its own `next_due` schedule (window.rs:413-421).
  A source that returns in under 0.15 period three times in a row, or fails, is demoted to the timer and retried every 5 s (window.rs:633-647, 621-625).
- **Phase lock** (`VsyncTimeline`, frametime.rs:14-105). Wake-ups are jittery (fraction of a ms to 3 ms), so raw times are never used for physics. `lock(raw)` snaps each wake-up to `last + n*period` where `n = floor(elapsed/period + 0.25)` (a wake-up up to 3/4 period late still belongs to its vblank, frametime.rs:90). Phase correction gain 0.1, period correction gain 0.02, period clamped to nominal +-3 percent (frametime.rs:31-33, 100-101). `n == 0` is a spurious second wake-up and is skipped (window.rs:650). Resync if the gap exceeds 30 periods (frametime.rs:29). An "off rhythm" score above 20 forces a re-read of the display rate (frametime.rs:38, window.rs:598, 618).
- **Frame stamping** (app.rs:3646-3656): `anim_t = tick.vblank + tick.period`, i.e. the animation is advanced to the time of the vblank the frame will be *shown* on, not the time it was drawn. `dt = anim_t - last_frame_t`, so each step is a whole number of refresh periods. Coming out of park, the first frame advances exactly one period, not the parked time (app.rs:3649-3653).
- **Display rate** is re-read every second while running and on `WM_DISPLAYCHANGE` / `WM_DPICHANGED` (window.rs:598, app.rs:1403). Nominal rate comes from `EnumDisplaySettingsW` (window.rs:345-359), not the measured one, so divisors never flip.
- **Idle cost**: the clock thread **parks on a Condvar** when not active (window.rs:586-594), zero CPU. The UI decides each frame whether to keep it running (app.rs:3963-3982):
  - `moving` (island unsettled, anim, repaint, outgoing fade, page switch, glass, queued commands, settings window drag) -> divisor 1 (full refresh).
  - Live equalizer (music) -> `ambient_divisor(hz, 60)`, i.e. every 2nd vblank at 144 Hz (frametime.rs:129-134).
  - Other ambient (drifting progress bar) -> about 30 fps divisor.
  - Otherwise parked; one-shot `SetTimer` wakes cover the next visible change (countdown second, HUD expiry, minute tick) (app.rs:4117, 4126).
  - Frames are also suppressed while locked or display off (app.rs:3960-3961), and fully skipped while hidden by fullscreen (app.rs:3617-3630).
- **Back-pressure**: `pending` flag so at most one `WM_APP_FRAME` is queued; a slow frame drops vblanks instead of queueing (window.rs:437, 659). The number of skipped vblanks is passed in `periods`.
- **Self-instrumentation**: `MotionStats` logs frames, fps, hitches (gap > 1.5 periods), worst gap, first-step time per morph; first 20 morphs always, then only on dropped frames, max one a minute (frametime.rs:136-195, app.rs:4020-4051). It also logs DWM counters (refreshes, composed, missed) and a per-frame breakdown: late ms, draw ms, present ms, mask ms (app.rs:4026-4036). This telemetry is part of why it feels smooth: they can see regressions.

What to learn: the "stamp with vblank + period" and "snap to period grid" trick is the key. The wasted-work part: it waits on vblank, then draws, then `Present(1)`; it is not using a waitable swap chain.

## 2. Window model

Files: `poni/src/window.rs`, `poni/src/gfx.rs`, `poni/src/app.rs`, `poni/res/poni.manifest`.

**Two windows, not one** (GPU path):
- **Island window** (window.rs:85-105): `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, `WS_POPUP`, class `Poni.Island`, `CS_DBLCLKS`. It receives all input. Its layered bitmap carries only an **invisible hit mask**.
- **Visual window** (window.rs:115-140): `WS_EX_LAYERED | WS_EX_TRANSPARENT | TOPMOST | TOOLWINDOW | NOACTIVATE`, **owned by the island** (so it stays just above it). `WM_NCHITTEST` returns `HTTRANSPARENT`. DirectComposition targets this window, not the island (gfx.rs:518-525 uses the hwnd passed in; the comment at window.rs:107-114 gives the reason).
- **Why two**: with DComp content on a layered window, touch ignores the layered alpha and the whole surface rectangle swallowed taps meant for windows beneath (window.rs:107-114). Splitting content (DComp) from input (layered bitmap alpha) fixes mouse, pen and touch click-through together. This is a real, hard-won Win32 gotcha worth copying.
- Plus a hidden **helper window** for tray, AppBar registration (fullscreen notification) and power messages (window.rs:236-243), and a separate borderless layered window for the Settings window with its own software `Gfx` (ARCHITECTURE.md).

**Fixed-size surface, the island morphs inside it**:
- Surface size = largest island (widest `expanded_width`, tallest page) x `OVERSHOOT` 1.05 + the island's side/bottom room (app.rs:582, 5673-5687). Window `w` is forced even so the centre lands on the monitor centre pixel (app.rs:5677-5680). The window **never resizes or moves during a morph** (app.rs:5683-5684 comment). Only re-laid out when page heights, scale or drag spot change.
- Island body is drawn centred (`Island::body_rect_at(cx)`, island.rs:259-262). Edges glide at sub-pixel positions because rendering is vector with DPI scaling.
- Dragged near a monitor edge, the window stops at the edge and the island keeps moving inside the window (`place_surface` / `place_island`, app.rs:899-935, using `poni_core::spot`).
- Where Y comes from: monitor top, or work-area top if a Windows taskbar is docked at the top (window.rs:289-304).

**Hit testing / click-through**:
- GPU renderer: layered bitmap alpha **1/255 inside the island shape, 0 outside** (gfx.rs:1374-1410, `mask_brush` black alpha 1/255 at gfx.rs:879). Collapsed = the island outline (+ shoulders), expanded = whole canvas rect (a forgiving margin of 11 DIP `HIT_MARGIN`) (island.rs:14, app.rs:5700-5714).
- Mask refresh is throttled: **30 Hz while the island moves, exact at rest** (`MASK_INTERVAL = 1/30`, app.rs:585; `update_hit_mask` app.rs:3991-4010), updating only the union of previous and new bounds (gfx.rs:1378-1383). Since the mask trails the outline, `over_stale_mask` ignores hover over parts the island already left (app.rs:3487-3500, 1147-1157).
- Software renderer: the frame itself is the layered bitmap, so any non-zero alpha takes the mouse; therefore the software path **draws no shadow** (shape.rs:89-93).
- Hover opens after a dwell: `hover_delay_ms` default 220, `collapse_delay_ms` 0; the dwell restarts when the pointer travels 4 DIP; re-entering within 0.3 s reopens from the current outline (app.rs:590-600, settings.rs:268-269).

**Topmost**: `WS_EX_TOPMOST` plus an active fight (app.rs:3320-3360, 80-81): after foreground changes a `raise_if_covered` runs, then 4 repeats every 500 ms, using `window_above()` to find the culprit class (logged once) and `SetWindowPos(HWND_TOPMOST, NOMOVE|NOSIZE|NOACTIVATE)`. Tracks pop-up classes allowed above it (menus, tooltips, Start/Search flyouts) in `services/fullscreen.rs:60-69`.

**Never activates**: text fields temporarily drop `NOACTIVATE` (`Command::FocusText`) and hand focus back (ARCHITECTURE.md).

**DPI**: `PerMonitorV2` in manifest (poni.manifest) and `SetProcessDpiAwarenessContext` (app.rs:127). D2D device context dpi = monitor dpi; all coordinates are DIPs. `WM_DPICHANGED` on the visual window is swallowed so it is never resized by Windows (window.rs:37). Body edges are pixel-snapped (`shape::snap`, shape.rs:66-71), hairline is whole pixels (`hairline`, shape.rs:75-78). Note `SHOULDER + HIT_MARGIN = 20` is kept even so the body edge lands on a whole pixel at 150 percent (island.rs:11-14).

## 3. Rendering

File: `poni/src/gfx.rs` (2242 lines), `poni/src/glass.rs`, `poni/src/services/backdrop.rs`.

**GPU path** (gfx.rs:410-540):
- `D3D11CreateDevice` hardware, falling back to WARP, with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`; feature level list 11_1 down to 9_1, retried without 11_1 (gfx.rs:443-468). WARP and the "Basic Render Driver" (Microsoft vendor 0x1414, device 0x8C) are both flagged soft (gfx.rs:431-441).
- **Direct2D 1.1 device context** (`ID2D1DeviceContext`) draws into the swap chain back buffer wrapped as `ID2D1Bitmap1` (TARGET | CANNOT_DRAW), re-created every `begin()` (gfx.rs:560-577). D2D factory is `SINGLE_THREADED`, DirectWrite `SHARED` (gfx.rs:876-877). Text AA grayscale (required for alpha), primitive AA per primitive (gfx.rs:283-284, 868-869).
- **Swap chain**: `CreateSwapChainForComposition`, `B8G8R8A8_UNORM`, **BufferCount 3**, `DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL`, `DXGI_ALPHA_MODE_PREMULTIPLIED`, `DXGI_SCALING_STRETCH`, flags 0 (not waitable, no tearing) (gfx.rs:503-517). Comment: three buffers so drawing never waits on DWM releasing one (gfx.rs:510).
- **DirectComposition**: `DCompositionCreateDevice` loaded at run time from System32 only (`LOAD_LIBRARY_SEARCH_SYSTEM32`), one target on the visual window, one visual whose content is the swap chain; committed once (gfx.rs:472-487, 518-525). No DComp animations or transforms are used.
- **Present**: `swap.Present(1, 0)` every frame (gfx.rs:1324). Device loss triggers fallback after repeated failures (`gpu_lost`, `gpu_failures` reset after 600 good frames, gfx.rs:1325-1331).
- **The "mask" step** has two meanings:
  1. *Hit mask* (section 2): an invisible alpha 1/255 bitmap on the layered island window, drawn with a separate **software** D2D target (`self.soft`), dirty-rect pushed with `UpdateLayeredWindowIndirect` (gfx.rs:1374-1410).
  2. *Shape mask for content during a morph*: `push_shape_mask` pushes a D2D geometric-mask layer of the island silhouette, anti-aliased (gfx.rs:1915-1946). It is applied only while `!settled || outgoing` (app.rs:3744-3747 comment "At rest every layout fits inside the body, and skipping the mask layer keeps ambient frames cheap").
- **Offscreen content layers** (`Canvas::layered`, gfx.rs:2099-2146): content that fades or scales during a morph is drawn into a surface-sized offscreen bitmap render target (layers cached per frame, `layers_used`) and composited with group opacity and a matrix transform. Benefits: correct group alpha (no see-through overlaps) and text rasterised once at rest size so it glides instead of re-hinting every frame (gfx.rs:2095-2096).
- **Software fallback** (gfx.rs:6-8, 1332-1338): D2D software target into a DIB, `UpdateLayeredWindowIndirect`, only the union of this and last frame's island rect copied. Used on Wine, no DComp, repeated device loss, or `PONI_RENDERER=software`.
- **Caches**: bitmap cache trimmed at 48 entries, text layout cache trimmed every 120 frames or past 1500 layouts (gfx.rs:1302-1309).
- **Shadow**: analytic ramp drawn with `rrect_shadow`, not a blur effect (section 5).

**Glass theme** (glass.rs, poni-core/src/glass.rs, services/backdrop.rs, `res/glass.hlsl`):
- What it is: "liquid glass" refraction. A thread owns its own D3D11 device on the island monitor's adapter and uses **DXGI Desktop Duplication** to copy the patch of the desktop under the window (services/backdrop.rs:1-20). Duplication only delivers a frame when the desktop changed; the thread copies the window rect and only wakes the UI if pixels really differ.
- The island window is excluded from capture with `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (both windows) so the copy never contains the island (window.rs:142-210, requires Windows 10 2004+, build 19041).
- The copy is uploaded to a texture and blurred twice with D2D Gaussian blur (sigma 3 DIP light frost, 9 DIP heavy frost, mixed by glass thickness) (glass.rs:19-21). An HLSL shader, **compiled at run time with System32's d3dcompiler_47.dll**, draws glass, rim light and shadow into a window-sized texture before each frame (glass.rs:1-12). Parameters include bevel, magnification, dispersion, saturation, light direction (glass.rs:30-70).
- Thickness: bevel 9 to 24 DIP, `mag = 0.85*bevel/1.6`, rim profile exponent 1.6, shadow reach 5 to 7, drop 1.5 to 3 between island heights 37 and 220 DIP (poni-core/src/glass.rs:6-55).
- Cost: undocumented numerically. It is gated: off while hidden, locked, display off, or **fullscreen** (draws "smoked glass" at 0.82 opacity instead so a game's frames are never copied), and it forces continuous frames while the backdrop changes (`glass_moving`, app.rs:3961-3968). Also turns the island invisible to screenshots, screen sharing and recorders while on (README line 251). Falls back to smoked glass on software renderer, Win10 < 2004, RDP, or laptop displays that cannot duplicate.

## 4. Springs

Files: `poni-core/src/spring.rs`, `poni/src/island.rs`, `poni/src/pill/motion.rs`, `poni/src/app.rs`.

**Model** (spring.rs:6-67): a plain damped harmonic oscillator, **unit mass**, parameters are `(stiffness k, damping c)` directly, not response/zeta:
`force = -k (x - target) - c v`, semi-implicit Euler, **fixed sub-step h = 1/240 s**, `dt` clamped to 0..0.1 s per call (spring.rs:54-61). Settled when `|x-target| < 0.05` and `|v| < 0.2` (opacity springs use eps 0.002, spring.rs:71-77; island.rs:243). Snap to target exactly when settled.

Presets: `bouncy` (420, 30), `smooth` (380, 2*sqrt(380) critical) (spring.rs:20-27). Test asserts bouncy overshoot between 0 and 15 percent and settle < 1.2 s (spring.rs:84-108).

**Classic (hanging) island tunings** (island.rs:20-29), (stiffness, damping), zeta = c / (2 sqrt k):

| Motion | k | c | zeta | note |
|---|---|---|---|---|
| Grow width | 420 | 30 | 0.73 | width leads |
| Grow height | 300 | 26 | 0.75 | height trails |
| Shrink width | 330 | 33 | 0.91 | |
| Shrink height | 420 | 38 | 0.93 | |
| Corner radius | 420 | 41 | 1.0 (critical) | "corners never wobble" |
| Content opacity | 700 | 52.9 | critical, tau about 38 ms | fade in |

**Pill/Diorama (floating) tunings** (`pill/motion.rs:21-40`), picked **per axis by that axis's direction** (so an axis that shrinks while the other grows is tuned correctly):

| Motion | k | c | zeta |
|---|---|---|---|
| Grow W | 500 | 32.5 | 0.73 |
| Grow H | 250 | 23 | 0.73 |
| Shrink W | 400 | 36 | 0.90 |
| Shrink H | 560 | 43 | 0.91 |
| Radius grow | 1200 | 69.3 | critical |
| Radius shrink | 150 | 24.5 | critical |
| Top gap (drop off edge, hover sink) | 300 | 34.6 | critical, tau about 58 ms |

Design intent in the comments: growing, width leads and height trails (stretch into a wide capsule, then drop into the card, about 3 percent overshoot, window leaves 5 percent); shrinking, height leads and width gathers last (the grow played backwards). A unit test pins every zeta to the comment within 0.006 (motion.rs:88-94).

**Kinds**: there is no separate "page" spring. Motion kinds are labelled only for the log (`motion_name`: pill, expand, collapse, page, resize; app.rs:5719-5725). A page switch is **not** a spring: it is a timed 0.26 s cross-fade/slide (`PAGE_SWITCH = 0.26`, app.rs:590) with `SLIDE = 20` DIP, old page gone by 40 percent, new page fades in from 10 percent over 50 percent, tab selection settles by 60 percent (pages/mod.rs:364-372). Outgoing content fades over 0.14 s with a smoothstep (app.rs:588, 620-623). Incoming content starts at scale 0.94 and 6 DIP up (2 DIP in floating mode) and settles as one layer (app.rs:613-614, 3866-3872; motion.rs:45). Delay before new content starts fading in: 0.03 s on grow, 0.05 s on shrink (island.rs:28-29). Ghost (leaving content) scales with the faster-shrinking axis, floored at 0.6 (app.rs:616; motion.rs:75).

**Interruption / retargeting** (this is the best part):
- Springs **keep value and velocity** on every new target; only `retune(k, c)` changes (island.rs:128-167, spring.rs:30-34, test `switching_mid_morph_retunes`).
- Retuning is decided from the position *at the moment of retarget*: `growing = target_area > current_area` for Classic; per axis for floating (island.rs:142-166).
- **Floor landing**: a shrinking axis records `floor = target` and is hard-clamped there (`land`, island.rs:316-325) so a spring turned around mid-flight never squashes the pill below its resting size; a growing axis whose spring is still falling keeps the old floor so it turns around above it (island.rs:303-314).
- Reopen within 0.3 s of leaving: re-enters from the current outline with no close-and-reopen (app.rs:595-598). Content that comes back mid-fade resumes from its current alpha (`resume_content`, island.rs:194-198; app.rs:3688-3712).
- A motion that starts from rest advances exactly one refresh period on its first frame, even if the previous frame was an ambient one (app.rs:3649-3653).
- Hover feedback is instant geometry, not a spring on a separate property: `hover_lift` outsets 3 DIP each side and sinks 1 DIP (floating, motion.rs:47-59); Classic grows 12 wide, 4 down (`HOVER_LIFT`, app.rs:602). It is just another target to the same springs.

## 5. Island geometry

- **Classic idle pill**: clock 84 x 30 (24 h) or 104 x 30 (12 h), radius 14, plus 22 or 34 for a battery badge; Minimal 150 x 26 r12; Eyes 118 x 30 r14 (36 tall with the Oyen theme); Hidden strip 150 x 5 r2.5 (activities.rs:262-329). Media pill 120 x 32 r15, peek 300 wide, timer 96/112 x 32 r15, HUDs 316 x 40 r19, Bluetooth 336 x 46 r22, notice 372 x 52 r24.
- **Pill/Diorama (floating) rest**: height **32**, radius = H/2 (capsule), clock 84/104 wide (152/180 with status), floats **8 DIP** below the top edge (`TOP_GAP`, pill/mod.rs:46; pill/idle.rs:20-60). Diorama rest is 190 x 38 r19 (pill/diorama.rs:534). Media pill width fits the lyric, 240 to 520, 8 DIP steps (pill/media.rs:41-52).
- **Expanded**: **620 default width** (`expanded_width`, clamp 480 to 900, settings.rs:270, 397). Height = header 50 + page content + padding 18, rounded (`HEADER_H = 50`, `PAD = 18`, pages/mod.rs:46-50; activities.rs:317-320). Content column 584 wide. Corner radius **30** Classic, **40** Pill (all four corners; pill/expanded.rs:23). A 620 x 216 card appears in tests.
- **Shape**: **no path interpolation and no superellipse.** The shape is a rounded rectangle (capsule/card) drawn from analytic arc segments each frame, with width, height and radius being three independent springs (island.rs). Classic outline is a notch: flat top, **concave shoulder arcs of radius 9** where it meets the screen edge, rounded bottom corners (`notch_segments`, gfx.rs:2150-2168; `SHOULDER = 9`, island.rs:8). The floating outline is `Seg` rounded-rect with radius clamped to a capsule (pill/shape.rs:46-61); corner never drawn rounder than half the height (island.rs:178-186, 246-250).
- **Shadow (floating, GPU only)**: analytic ramp encoding a key shadow (black 0.32, 4 down, Gaussian sigma 6) and a contact shadow (0.20, 1 down, sigma 1.5) as one 12-stop piecewise ramp, cut at 15 DIP beside and 19 below; fades with the fill (pill/shape.rs:14-42, 97-112). Window margin 20 DIP (shape.rs:14). Classic has no shadow. 1 DIP hairline border `white(0.07)` just inside the edge, whole pixels at any scale (shape.rs:73-119; palette BORDER).
- **Diorama frame**: a 5 DIP max bezel growing with the shorter side from the pill's 38 to 150 DIP (shape.rs:121-140).

## 6. Visual design

- **Palette** (`poni-core/src/geom.rs:171-198`): island pure black `#000`; hairline white 0.07; text white 0.96 / 0.62 (dim) / 0.50 (faint, about 5:1 on black) / disabled 0.30; surface white 0.08, hover 0.15, pressed 0.22; track 0.16. Accents are Apple dark system colours: green `#30D158`, orange `#FF9F0A`, red `#FF453A`, blue `#0A84FF`, cyan `#5AC8FA`, purple `#BF5AF2`, yellow `#FFD60A`, pink `#FF375F`. "On accent" white is 0.998 blue so looks can repaint it.
- **Type** (gfx.rs:145-152, 805-822): DirectWrite shared, family picked from `Segoe UI Variable Text` then `Segoe UI Variable` then `Segoe UI`. Scale **11 / 12 / 13 / 15 / 18 / 30 / 50** (caption, footnote, body, headline, value, display, hero); nothing under 11. `tabular()` for fixed-width digits. Oyen theme uses **Outfit** as the brand face. Marketing site tokens list SF Pro, Inter fallback (design/tokens.json).
- **Icons**: original SVG path data on a 24 x 24 grid, 2-unit round-capped stroke, non-zero fill, parsed once and cached (icons.rs:1-12); about 60+ icons including AI service marks. Drawn by the renderer's own path parser (`poni-core/svgpath.rs`).
- **Buttons**: `ui.pill(...)` styles Utility / Neutral / Primary(c) / UtilityAccent(c) at 28 or 34 high, 8 gap.
- **Page/tab structure**: header row of tabs (50 DIP) with a sliding selection pill, gear right of the tabs for Settings; tabs: Music, Today, Shelf, Tasks, Focus, AI, Oyenza, plus add-on tabs (Health, IELTS, Notes, System, Script, Record, Prayer, Stocks, Caffeine, Downloads, Notifications) and a final "+" Add-ons tab (pages/mod.rs:59-99; README). Immediate-mode UI with `ui.button(id, rect) -> Resp`, ids hashed per frame; clicks arrive the frame after mouse-up. Mouse wheel over the header switches tabs with a 0.3 s debounce (app.rs:3900-3911).
- **Idle styles** (settings.rs:8-22): Clock (clock + battery ring), Minimal (blank), Eyes (blinking), Hidden (invisible strip until the cursor arrives), Status (wide status-bar pill, Pill theme).
- **Themes** (settings.rs:26-46): **Classic** (plain black notch hanging from the edge), **Oyen** (cream ground, dark brown text, orange accent, Outfit font, the cat peeks over the edge, bobs to music, sleeps midnight to 05:00), **Glass** (liquid glass, section 3), **Pill** (floating 8 DIP below the top as a capsule, own layouts, card radius 40), **Diorama** (Pill plus a pixel-art window at rest: house, tree, clock-driven sky, a glass ball that rolls at sunset; `studio-diorama` crate, painted once a minute as an image, never per frame). Themes change look and geometry only; pages and activities are shared.
- **Light/dark**: the island is always dark; the Settings window follows the system, using `look::glass(light)` to flip the island's white ink to dark ink.
- **Content motion polish**: group-opacity layers, pivot is the small end of the morph so content grows out of the pill and shrinks into it (motion.rs:61-69).
- **Window self-protection**: Settings and Script pages exclude the island from capture; recording hides it from its own video.

## 7. Shelf, integrity, fullscreen

**Drag in** (`poni/src/dragdrop.rs`): in-process **OLE drop target**. `OleInitialize` on the UI thread (STA) (app.rs:136), `RegisterDragDrop(island_hwnd, IDropTarget)` (dragdrop.rs:205). Accepts `CF_HDROP` (dragdrop.rs:41, 168-190), uses the shell `IDropTargetHelper` for the drag image over the window (dragdrop.rs:196). Effect: **copy by default, link with Alt, never move** (the shelf stores references, not copies; dragdrop.rs:63-71). `DragHover` event opens the Shelf while a file drag is over the island, and a hold keeps it open until a 380 ms grace after the drag ends (app.rs:601).
**Drag out**: modal `DoDragDrop` from a deferred command, UI thread (dragdrop.rs:251-295). Items are resolved with `SHParseDisplayName`, bound to a `IShellItemArray`, then `BHID_DataObject` for a real shell data object, so Explorer, mail and chat apps get the genuine shell data (thumbnail, drop descriptions via `IDragSourceHelper2` with `DSH_ALLOWDROPDESCRIPTIONTEXT`). Files moved or deleted are skipped. `DRAGGING_OUT` stops the island refusing its own drag. Shelf size limit `MAX_SHELF_ITEMS` in `poni_core::shelf`. A related feature: Phone drop, an embedded LAN web server (poni-core/src/phonedrop) plus str0m WebRTC (PairDrop style) for phone transfers.

**Integrity / elevation**: manifest `requestedExecutionLevel level="asInvoker" uiAccess="false"` (res/poni.manifest). So the app runs at **medium integrity, no UIAccess**. Admin is used only transiently: the one-time "Show over games" OK re-runs `poni.exe --allow-fps <SID>` elevated to add the account to the Performance Log Users group (services/frames.rs:16, 408, 524). No signing: README line 181 says the Windows builds are not code-signed, so SmartScreen warns. MSVC builds link with `/DEPENDENTLOADFLAG:0x800` (System32-only DLL load, build.rs).
Because it has no UIAccess, it is **not** above elevated windows' input boundary and cannot sit over the secure desktop or UAC. It shows topmost only by the topmost fight in section 2. It can be covered by "exclusive" fullscreen.

**Fullscreen** (`services/fullscreen.rs`, 874 lines): event-driven, no polling. Signals: shell `ABN_FULLSCREENAPP` via an AppBar registration that never calls `ABM_SETPOS` (so reserves no space), a global out-of-context `EVENT_SYSTEM_FOREGROUND` WinEvent hook, `EVENT_OBJECT_LOCATIONCHANGE` hooks scoped to the foreground window's thread (so F11 in a browser is noticed without every window's churn), plus `SHQueryUserNotificationState` (QUNS_BUSY, PRESENTATION_MODE, RUNNING_D3D_FULL_SCREEN). Shell classes (desktop, taskbars, Alt+Tab host) are excluded by a class list (fullscreen.rs:57-67). Setting `hide_in_fullscreen` defaults **true** (settings.rs:271): the island is not drawn, timers still tick (app.rs:3617-3630). **Show over games**: borderless games only, counts FPS from the ETW frame log the way PresentMon does (DX9-12 Present; Vulkan/OpenGL from the kernel log), reads GPU/CPU via the System page counters, can show a persistent readout or "peeks" a few seconds, Alt+Shift+F toggles; README warns that any window over a borderless game makes Windows composite it (about one frame of latency), hence the Peeks mode (README lines 287-291). It is explicitly **not** over exclusive fullscreen. "Step aside" (disabled by default since 0.2.6) detects maximised windows under the island and rests as a hidden strip.

## 8. Clever things to learn, and weaknesses

### Clever
1. **Vblank-stamped time**: advance physics to `vblank + period`, snap wake-ups to the period grid with a leaky phase/period tracker, accept "late only" wake-ups with a +0.25 period bias. The idea that every step is an exact integer of refresh periods is what makes 60/144/240 Hz equally smooth.
2. **One-refresh first step** on resume from park, so the first frame never jumps by the parked time.
3. **Divisors from the nominal refresh rate** (not measured) for ambient motion.
4. **Per-axis spring retune on retarget** with preserved velocity, plus **floor landing** to prevent squash on interrupted shrinks. Plus resume-content-from-current-alpha. This is the difference between "springy" and "alive under interruption".
5. **Fixed-size surface + centre on one pixel + even width**: no HWND moves or resizes, DPI-snapped body edges and hairline.
6. **Visual window / hit-mask window split** for touch-correct click-through with composition content.
7. **Hit mask updated at 30 Hz with dirty-rect union**, plus explicit "stale mask" suppression.
8. **Content as an offscreen layer during morphs**: crisper, group alpha, no per-frame text re-hinting; masks only while morphing.
9. **Analytic shadow ramp** fit to a two-shadow Gaussian model, instead of a blur effect.
10. **Self-measuring**: per-morph hitch stats, DWM missed-frame counters, a `selftest.rs` of 5602 lines that drives the app headless (CI "poni-screens" artifact with `smoke-out/metrics.txt` memory at rest, AGENTS.md rule 16). Every PR must report installer size, memory at rest, per-frame cost.
11. **Idle discipline**: parked clock thread, one-shot timers for the next visible change, no polling for fullscreen, services that stop when locked/display off/battery saver, features loaded lazily (AGENTS.md lines 7-16). Reported claims: 2.4 MB Windows download (README:128).
12. **Theme layer is geometry + look only**; pages are theme-agnostic.

### Weaknesses and gaps (what we can beat)
1. **All motion is CPU-drawn on the UI thread.** A stall in any page's draw, service event or `run_commands` stalls the morph. DirectComposition is used only as a pass-through for one swap chain, never for transforms/animations (gfx.rs:518-525, no `IDCompositionAnimation`). A compositor-driven scale or opacity for the final settle would be immune.
2. **Two windows, 30 Hz mask**: the mask trails the outline up to about 33 ms, so edge hover is imprecise during motion (they added `over_stale_mask` to hide this). A single window with a non-layered DComp/`WS_EX_NOREDIRECTIONBITMAP` approach and a region (or `WM_NCHITTEST` + a small mask) could be exact.
3. **Window-sized swap chain and layer bitmaps**: the surface is as large as the tallest page at max `expanded_width` times 1.05 (roughly 985 x 600+ DIP, more at 150 to 200 percent DPI), the swap chain has 3 buffers of that size, and `Canvas::layered` allocates surface-sized offscreen targets. The memory and the DWM compose area are far larger than the collapsed island. Not measured by us; Oyenza's own CI logs the figure in `smoke-out/metrics.txt` (not in the repo).
4. **`Present(1)` with no waitable swap chain / frame latency control**; one frame of queue is plausible. Draw is done after the vblank wake-up, so effective latency is vblank wait + draw + compose.
5. **Fullscreen**: hides in fullscreen by default; borderless-only overlay; exclusive fullscreen covers it; no UIAccess, no DLL injection (a design choice for anti-cheat safety, but it means Show over games gets one frame of added DWM composition latency and needs an admin OK for ETW). Not signed.
6. **Elevated apps**: medium integrity and no UIAccess means clicks on the island over an elevated foreground window may be blocked by UIPI and the island cannot reliably stay over elevated or secure-desktop surfaces. (Inference from the manifest; not tested.)
7. **No audio-routing intelligence**: no Voicemeeter, no per-app mixing; volume HUD only (README). No game-controller or overlay window class beyond FPS readout.
8. **Scope creep**: 135k lines of Rust in one workspace, 5.9k-line `app.rs`, 5.6k-line `selftest.rs`, features such as IELTS, stocks, prayer times, caffeine and a phone WebRTC drop. More surface to regress, higher memory and installer size despite lazy loading. README states 2.4 MB download, but add-ons bring yt-dlp 23.6 MB when used.
9. **Fixed-size windows hurt multi-monitor and ultra-wide**: the window is laid out per monitor and re-laid out on monitor change (`place_surface`), which is the only moment the surface can resize.
10. **Page switching is a timed tween, not a spring**, so interrupting a page switch does not carry velocity (page_anim is `(from, t0, dir)` with a fixed 0.26 s, app.rs:3751-3756).
11. **Glass needs Desktop Duplication**: unavailable over RDP, some laptops, Win10 < 2004; blurs a copy of the screen, so it lags the real desktop by a frame or more; hides the island from capture (README:251).
12. **Windows builds unsigned** (README:181) and a second process path (`--allow-fps` elevated) is used for the FPS feature.
13. Single-threaded D2D factory and text layout caches on the UI thread; no mention of MMCSS or thread priority boosts for the frame clock (grep found none in `app.rs`, `main.rs`, `window.rs`).

## 9. Concrete numbers to beat

- Spring baseline: grow zeta 0.73 (about 3 percent overshoot), shrink zeta 0.9, radius critical; settle in under 1.2 s in the unit test (spring.rs:95). Expand logs appear as "44 frames in 302 ms (143.7 fps at 144 Hz), 0 hitches, worst gap 7.1 ms" (frametime.rs:181 doc example, an example, not a measurement of our own).
- Hit mask: 30 Hz while moving. Beat it with 1:1 per frame.
- Rest pill 32 DIP high floating 8 DIP below the edge, card 620 wide, radius 40.
- Content crossfade: 0.14 s out, 0.26 s page switch, hover dwell 220 ms.
- Idle: zero frames, one parked thread; minute-tick timer.
- Memory at rest and CPU in motion: not measurable from source; measure the installed binary side by side (Task Manager private bytes, `QueryProcessCycleTime`, PresentMon for dropped frames at 144/240 Hz) before claiming parity.

## 10. Source map

| Topic | File |
|---|---|
| Spring | `poni-core/src/spring.rs` |
| Frame timeline | `poni-core/src/frametime.rs` |
| Frame clock, windows | `poni/src/window.rs` |
| Island springs | `poni/src/island.rs` |
| Floating tunings | `poni/src/pill/motion.rs` |
| Shadow, outline | `poni/src/pill/shape.rs` |
| Frame loop, hit mask, surface | `poni/src/app.rs` (5896 lines) |
| Renderer, swap chain, DComp | `poni/src/gfx.rs` |
| Glass | `poni/src/glass.rs`, `poni-core/src/glass.rs`, `poni/src/services/backdrop.rs`, `res/glass.hlsl` |
| Palette | `poni-core/src/geom.rs:171` |
| Settings and themes | `poni-core/src/settings.rs` |
| Pages and tabs | `poni/src/pages/mod.rs` |
| Drag and drop | `poni/src/dragdrop.rs` |
| Fullscreen | `poni/src/services/fullscreen.rs` |
| Manifest | `poni/res/poni.manifest` |
