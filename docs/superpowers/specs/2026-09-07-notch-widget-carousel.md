# Phase 6 slice 3 — Notch widget carousel

**Status:** design agreed 2026-09-07, from a live session driving the slice-2 build.

**Depends on:** slice 2, and specifically the three fixes that made the panel usable at all —
the `{x:Static}` crash (`6469ef1`), mouse input reaching the window (`1c60e5a`), and the
click-through / surface-size pair (`6277c8a`).

---

## Problem statement

Hovering the notch opens one fixed panel: a single ambient row above whatever cards the OSD
was already going to render. There is one view and no way to reach anything else, so the
notch can only ever show what fits on one line.

The reference apps do not work that way. The notch is a small surface you **page through**:
a home view, a calendar, media, a file shelf — one horizontal gesture apart. That is what
makes an always-present strip worth having rather than a second volume popup.

The user's words, driving this: *"yukarıda sağa doğru ya da sola doğru touchpad ile
yaptığında değişiyor, diğer widget'a geçiyormuş gibi — mouse'ta da bunu iyi planlaman
lazım"*, and for the home page *"saat tarih ve sağda weather"*.

## Goals

1. The open panel is a **pager**. Page 1 is home; further pages hold a calendar and whatever
   later slices add. Paging is horizontal and animated.
2. **Home page**: clock and date on the left, current weather on the right, each taking half
   the width. No wasted middle.
3. **Input works on a touchpad and on a mouse.** A two-finger horizontal swipe and a mouse
   with no horizontal wheel must both be able to change page.
4. **Weather looks alive** — motion that suits the condition, not a static glyph.
5. Classic OSD is untouched, as in every slice so far.

## Non-goals

- The file shelf. Still deferred, still gated on the drag-and-drop spike from slice 2's spec.
- Notifications. Declined by the user in slice 2 and still declined.
- Making the notch a launcher, a clipboard manager, or a settings surface. It pages between
  glanceable widgets; that is the whole scope.

---

## §1 — The pager

A `NotchPager` control hosting an ordered list of widget views, showing exactly one at a time,
with a horizontal slide between them.

**Pages are not cards.** `CardHost`/`ICard` decides what the *OSD* shows for an event and is
the single authority for when the OSD appears — that contract stays exactly as it is. The
pager lives **inside the ambient home content** that a hover opens, and an event-driven show
never touches it. A volume key still opens the volume card, not page 3 of a carousel.

Page state is per-open-session by default: opening the notch returns to page 1. A "remember
the last page" setting is deliberately not in this slice; it is easy to add later and easy to
regret shipping before anyone has used the paging.

**Animation.** The outgoing page translates out and the incoming one translates in, both on a
single progress value, exactly as `NotchExpand` drives the notch's own morph. That decision is
not stylistic: parallel clocks on one visual transition can be stopped, retargeted or completed
independently, and this branch has produced five defects from state that depended on a single
animation callback firing. One clock cannot desynchronise from itself.

## §2 — Input, which is the hard part

Windows reports a touchpad's two-finger horizontal scroll as **`WM_MOUSEHWHEEL`**, and a mouse
with a tilt wheel reports the same message. One handler therefore covers both, which is the
whole reason to build on it rather than on gesture APIs.

Three routes, one code path behind them:

| Route | Message | For |
|---|---|---|
| Two-finger horizontal swipe | `WM_MOUSEHWHEEL` | Touchpads |
| Tilt wheel | `WM_MOUSEHWHEEL` | Mice that have one |
| `Shift` + vertical wheel | `WM_MOUSEWHEEL` with `MK_SHIFT` | Every other mouse — the established Windows convention for "scroll horizontally" |

Plus **clickable page dots** under the content, which are also the affordance that tells a
first-time user paging exists at all. They are the only route that needs no gesture knowledge,
so they are not optional.

**`WM_MOUSEHWHEEL` does not route through WPF's normal input events.** It must be handled in
the band window's `WndProc` and forwarded. That is a real constraint and it is why input gets
its own task rather than being folded into the pager's.

**Wheel deltas are not page counts.** A touchpad emits a stream of small deltas for one
physical swipe; treating each as a page turn would fly through every widget. Accumulate delta
and commit one page per threshold, then require the accumulator to return near zero before the
next commit. The threshold and the reset belong in a pure, unit-tested function — the same
gather/decide split every other subsystem in this phase uses.

**Verify on the user's actual hardware.** Touchpad drivers differ in delta magnitude and in
whether they emit inertia after the fingers lift. Inertia that keeps arriving after a page
commit is exactly what makes a carousel feel broken, and it cannot be reasoned about from here.

## §3 — Home page layout

```
┌────────────────────────┬────────────────────────┐
│  21:32                 │      ☁  22°            │
│  Pazartesi             │      Partly cloudy     │
│  7 Eylül               │      ↑24°  ↓14°        │
└────────────────────────┴────────────────────────┘
```

Two equal columns. Clock, weekday and date left; condition glyph, temperature, label and the
day's range right. Battery moves to a small badge beside the clock rather than a third column —
it is one short value and does not deserve a half.

The existing single-row `AmbientCardView` is replaced by this as page 1. `AmbientCardViewModel`
keeps its role and its `AccessibleSummary` contract; only the view changes.

## §4 — Calendar page

Current month, the week's days as headers, today marked. Read-only in this slice: no event
list, no navigation between months. A calendar you can only look at is worth shipping; one
that half-navigates is not.

Event data would mean a provider integration and a permission story, and neither belongs in
the slice that introduces paging.

## §5 — Weather motion

Condition-driven, built from WPF `Storyboard`s with no new dependency:

| Condition | Motion |
|---|---|
| Clear | A slow rotating glare behind the sun glyph |
| Partly cloudy / overcast | Clouds drifting slowly across, wrapping |
| Drizzle / rain / showers | Drops falling, staggered, recycled |
| Snow | Flakes drifting with slight horizontal sway |
| Thunderstorm | Rain plus an occasional brief flash |

**Every storyboard stops when the panel closes.** The OSD's window is never hidden in notch
mode, so an animation left running is a permanent CPU cost on an overlay that is supposed to be
invisible most of the time. Slice 1's ledger already carries an unmeasured idle-resource item;
this must not be what finally makes it matter. Start on open, stop on collapse, and measure it.

## §6 — The resting shape

Two changes, and the first is not cosmetic.

**Raise the resting height.** The default is 5 DIP and the reporting user runs 2. At that size
the notch is invisible *and* it is not a physical target: because the OSD is a layered window
hit-tested against its alpha, a two-pixel strip is two pixels of clickable surface in an
otherwise transparent window. That is the mechanism behind slice 2's keep-alive defect. A
resting height around 30 DIP — what the reference apps use — makes the shape read as a notch
and makes the target real. Keep the setting; change the default and widen the range.

**Make the notch surface opaque.** It currently uses the theme's `OsdSurfaceBrush`, which reads
as a floating card near the top of the screen rather than as part of the bezel. A notch is
near-black. This forces the content colours dark in notch mode too, or a light-theme user gets
dark text on a dark surface — that coupling is the reason it was deferred out of slice 1, and
it needs handling here rather than deferring again.

## §7 — What must be verified live, and why the automated gates cannot

Slice 2 shipped green on build, 221 tests and the accessibility lint, and still crashed on the
first hover, then failed to accept a single click, then drew half its content over the desktop.
None of those were reachable from the suite: it is not STA, so it cannot construct a
`UserControl`, load a template, or receive a mouse message.

Everything in this slice is in that same blind spot. The gates prove it compiles and that the
pure logic is right. They prove nothing about whether it works.

**Install and drive it after every task.** Specifically: page with a touchpad, page with a
mouse, click the dots, watch a full animation cycle, and check the panel still accepts clicks
afterwards.

**Instrument the process; do not sample it from outside.** Three external sampling harnesses
were written during slice 2's investigation and all three produced misleading or empty results,
while a four-line diagnostic inside `OsdHost` answered the question immediately.

## §8 — The defect class this slice must not repeat

Five defects on this branch share one shape: **state derived from a single moment, or a single
callback, that turned out not to fire.**

- `BeginAnimation(prop, null)` removes a clock without raising `Completed` — four times.
- One synchronous `Measure` read a tree the `ItemsControl` had not materialised yet.

Both were fixed the same way: stop trusting the event, and recompute the answer where it is
used. The pager's page index, its animation state and any hit-testing it introduces must be
written the same way — derived where they are consumed, not cached from a callback.

## Deferred

- The file shelf, still behind its drag-and-drop spike.
- Remembering the last page across opens.
- Calendar events, and any provider integration behind them.
