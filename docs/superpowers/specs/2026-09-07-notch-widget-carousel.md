# Phase 6 slice 3 — Notch widgets

**Status:** design agreed 2026-09-10 against a live mockup the user drove directly.
Supersedes the first draft of this file, which described a single home page with the clock
and weather side by side.

**Depends on:** slice 2, and the fixes that made the notch usable at all — the `{x:Static}`
crash (`6469ef1`), click-to-open replacing hover-to-open, and the window architecture change
that finally stopped a closed notch blocking the desktop (`0f1fda7`).

**Mockup:** every state below is live at
`docs/superpowers/design/notch-design.html` — hover peeks, click opens, a horizontal wheel
pages, the volume track drags, and the weather page's first-look reveal has its own control.

---

## Problem statement

The notch opens one fixed panel: an ambient row above whatever cards the OSD was going to
render. There is one view and no way to reach anything else, so it can only ever show what
fits on one line.

The first attempt at fixing that put the clock and the weather side by side on one "home"
page. Driving it on a real surface killed the idea in one sentence from the user: *"saat ve
weather olunca çok büyük oluyor"* — a notch that opens into a wide slab stops reading as a
notch.

## Goals

1. **One widget per page**, paged sideways.
2. **One fixed frame.** Every page opens to the same size.
3. **Widgets do things.** The volume level drags; the transport buttons work.
4. **Weather is the sky**, edge to edge — not a glyph beside a number.
5. Classic OSD untouched, as in every slice.

## Non-goals

- Notifications. Declined twice.
- Remembering the last page across opens. Easy to add, easy to regret before anyone has used
  the paging.
- Calendar events, and the provider integration behind them.

---

## §1 — Two shape families

The notch is never a card. It takes one of two shapes, and **which shape it takes says who
started the interaction**.

| | **Event HUD** | **Widget page** |
|---|---|---|
| Trigger | A volume key, a track change | A click on the resting notch |
| Shape | Short and wide — `300 × 46`, `372 × 54` for media | The fixed frame — `356 × 116` |
| Lifetime | ~2 s, then gone | Held while in use |
| Page dots | None. It is not a place. | Yes |

This replaces the current arrangement, where an event opens the same panel a hover does and
the volume card is just another row in it. An answer to something you did should not look like
a place you went.

## §2 — One frame, whatever is in it

**Every widget page opens to exactly `356 × 116`.** Content moves through the frame; the frame
does not move.

Two earlier passes did it the other way and both were rejected on the mockup:

- **Both dimensions per page** — swiping made the notch itself jump around. Read as instability.
- **Height only** — better, but the bottom edge still moved on every page turn.

The clock page carries empty space as a result. That is the correct trade: a clock centred in a
steady frame reads as deliberate, a frame that shrinks to hug it reads as unstable.

Page transitions slide: the incoming page enters from the side you swiped from, so paging reads
as movement through a frame rather than a crossfade in place.

## §3 — The widgets

| Page | State | Notes |
|---|---|---|
| **Clock** | built | Time and date, centred. |
| **Weather** | new | Full-bleed sky — see §4. |
| **Media** | partly built | Art, title, working transport. |
| **Audio** | new | Draggable level, endpoint and Voicemeeter bus. |
| **Shelf** | behind a spike | Drag files onto the notch to hold them. |

**Audio is the one worth building first among the new ones.** Plith already knows every
rail — the endpoint, the bus, the level — and no other notch app has an audio engine
underneath it. A draggable level in the notch is the thing only this app can offer.

**Interactive, not decorative.** A widget you can only read is a notification with extra
steps, and §1 already gives notifications their own shape.

## §4 — Weather: the sky is the page

Not an icon beside a number. **The panel's entire background becomes the sky**, the way
Samsung's weather app does it — which matters more here, not less, because the surface is
small enough that a 60 DIP glyph beside a temperature wastes most of it.

- **Gradient** from condition and time of day: day, overcast, rain, night.
- **Sun blooms rather than spins.** At this scale a rotating ray wheel reads as a loading
  spinner. It pulses slowly and sits off-centre, partly out of frame.
- **Clouds sweep** across the full width on long, staggered loops.
- **Rain and snow fall** as generated elements, staggered, recycled.
- **A scrim under the text**, so the numbers stay legible whatever the sky is doing. Without it
  the temperature disappears into the rain gradient.

### First look of the day

The first time the weather page is opened on a given day, the sky **arrives**: the gradient
swells in, the sun rises into frame, cloud sweeps across — about 1.5 s — and then settles into
the ambient loop it keeps for the rest of the day.

**An event the first time, wallpaper every time after.** A flourish that fires on every glance
stops meaning anything, and on a surface that lives at the top of the screen it would go from
delightful to tiring inside a day. The trigger is a stored date, not a session flag: opening
the notch fifty times before noon should produce exactly one reveal.

### Motion has a cost and it is measured

**Every storyboard stops when the panel closes.** The OSD's window is never hidden in notch
mode, so an animation left running is a permanent CPU cost on an overlay that is invisible most
of the time. Start on open, stop on collapse, and measure it — slice 1's ledger already carries
an unmeasured idle-resource item, and this must not be what finally makes it matter.

## §5 — Input

A touchpad's two-finger horizontal swipe and a mouse tilt wheel produce **the same message**,
so one handler covers both. That is the whole reason to build on it rather than on gesture APIs.

| Route | Message | For |
|---|---|---|
| Two-finger swipe | `WM_MOUSEHWHEEL` | Touchpads |
| Tilt wheel | `WM_MOUSEHWHEEL` | Mice that have one |
| `Shift` + wheel | `WM_MOUSEWHEEL` + `MK_SHIFT` | Every other mouse |
| Page dots | click | The affordance that says paging exists |

**`WM_MOUSEHWHEEL` does not route through WPF's normal input events.** It has to be taken in
the window's message hook and forwarded — which is now `HwndSource.AddHook`, since WPF owns
the window after `0f1fda7`.

**Wheel deltas are not page counts.** One physical swipe emits a stream of small deltas;
committing a page per delta would fly through every widget. Accumulate, commit one page per
threshold, then require the accumulator to fall back near zero. That decision belongs in a
pure, unit-tested function — the same gather/decide split every other subsystem in this phase
uses.

**Verify on the user's own hardware.** Touchpad drivers differ in delta magnitude and in
whether they emit inertia after the fingers lift. Inertia arriving after a commit is exactly
what makes a carousel feel broken, and it cannot be reasoned about from here.

## §6 — What the automated gates cannot see

Slice 2 shipped green on build, 221 tests and the accessibility lint, and then crashed on the
first hover, refused every click, and drew half its content over the desktop. None of it was
reachable from the suite: it is not STA, so it cannot construct a `UserControl`, load a
template, or receive a mouse message.

Everything in this slice sits in that same blind spot. **Install and drive it after every
task** — page with a touchpad, page with a mouse, click the dots, drag the volume track, watch
a full sky cycle, and check the desktop underneath still takes clicks afterwards.

**Instrument the process; do not sample it from outside.** Three external sampling harnesses
were written during slice 2's investigation and all three gave misleading or empty results,
while four lines inside `OsdHost` and a message counter in the window hook answered each
question immediately.

## §7 — The defect class this slice must not repeat

Every defect on this branch shares one shape: **state derived from a single moment, or a
single callback, that turned out not to fire.**

- `BeginAnimation(prop, null)` removes a clock without raising `Completed` — five times.
- One synchronous `Measure` read a tree the `ItemsControl` had not materialised yet.
- A parked flag written in a completion handler left a closed notch believing it was open.

All were fixed the same way: stop trusting the event, recompute the answer where it is used.
The pager's page index, its slide state and the sky's first-look flag must be written that way.

## Deferred

- The file shelf, behind its drag-and-drop spike.
- Remembering the last page across opens.
- Calendar events.
- Refining the cloud shapes and the snow — the mockup's are placeholders good enough to agree
  the direction, not to ship.
