# Notch Output Picker

**Date:** 2026-09-21
**Branch it builds on:** `feature/shelf-drop-catcher`
**Spec this refines:** `docs/superpowers/specs/2026-09-20-media-widget-alcove-design.md` (§ the output button)

## Why this change has the shape it has

The media page's output control opens `ms-settings:sound`. That was chosen deliberately and
recorded as a compromise: changing the default render endpoint has no documented API, so the
alternative would have needed "a popup surface the notch does not have".

The user's own counter-proposal removes that reason: **the page itself becomes the list.** No
second window, so the notch keeps its shape, and the way back is a control rather than a window
closing. That is worth acting on, because a `Popup` or `ComboBox` would have been a genuine trap
here and not merely inelegant: WPF opens both in their OWN window, which is not part of the
notch's layered per-pixel-alpha surface, so it would draw as a square rectangle outside the
notch's shape and theme. `docs/PHASE6-VERIFICATION.md` §14 records what this window's hit
testing costs to get wrong: "the click-through problem, SOLVED, after eight attempts".

### What was measured first

**Five active render endpoints on this machine, and two of them were indistinguishable.** Asked
through Plith's own `WindowsAudioClient.EnumerateRenderEndpoints()`:

```
Hoparlör (Steam Streaming)      <-- same
Hoparlör (Realtek(R) Audio)
Hoparlör (Steam Streaming)      <-- same string, different id
PG27AQDM (NVIDIA High)
Hoparlör (Logitech G733)
```

Asked again without Plith's shortener, straight from Core Audio, the names are all distinct:

```
Hoparlör (Steam Streaming Speakers)
Hoparlör (Realtek(R) Audio)
Hoparlör (Steam Streaming Microphone)      a render endpoint, with "Microphone" in its name
PG27AQDM (NVIDIA High Definition Audio)
Hoparlör (Logitech G733 Gaming Headset)    the current default
```

So **the collision is Plith's own**, not the machine's. `AudioLabel.Shorten` keeps the adapter's
first two words, and "Steam Streaming Speakers" and "Steam Streaming Microphone" both reduce to
"Steam Streaming". This is not hypothetical and it is not new: `SettingsWindow`'s endpoint combo
box is built from the same call, so **it shows two identical rows today**.

The roadmap's own note on this ("every render endpoint begins Hoparlör (, so trimming to fit
produces the same useless string for all five") was measured during the System Controls work and
is right about the symptom. The cause is narrower and fixable.

**The switching code does not exist.** `docs/ROADMAP.md` records that `SetDefaultEndpoint`
returned `S_OK` here, and the revert commit `7dddfc8` repeats it. Searching the whole history for
`IPolicyConfig`, `PolicyConfig`, `870af99c` and `SetDefaultEndpoint` finds only prose: the
verification was a probe, and no committed file ever held the interop. The measurement stands;
the implementation has to be written and re-measured.

## Goal

A person on the media page presses the output control and the page becomes a list of the outputs
they can switch to, with the current one marked. They press one, the system output changes, and
the page goes back to what is playing. Nothing opens a second window, and no device is hidden
behind a count.

## Decisions taken, and why

1. **A mode of the media page, not a page of its own.** A separate widget page would join the
   pager's rotation, so a swipe would land on a device picker, which is not a place anyone was
   going. As a mode, the page swaps its own content and the way back is a named control rather
   than a page turn.

2. **Two columns, three rows: six cells.** The user's choice from the mockups. The alternative
   offered was one full-width column with the notch's own pager continuing it, and it was
   rejected for a good reason: paging already means "next widget", and one gesture with two
   meanings is a gesture nobody can predict.

3. **Capacity is the product of the grid, and the overflow has a named door.**
   `NotchGeometry.OutputPickerCapacity` is `columns * rows`, defined as the product for the same
   reason `ShelfCapacity` is: a literal beside a grid is free to stop matching it. With more
   endpoints than cells, the LAST cell becomes "More in Windows settings" and opens
   `ms-settings:sound`, so five devices are drawn and the rest have a door rather than a silent
   fold. A folded tile is in no UIA tree at all, which is the shelf's own most expensive lesson.

4. **The current default comes first, then enumeration order.** The one you are on is the
   anchor. Pure, so it is testable.

5. **Console and Multimedia roles are written. Communications is left alone.** That is what
   Windows' own "Set as Default Device" does, while "Set as Default Communication Device" is a
   separate action. Moving the communications default would move Discord and Teams audio, which
   nobody asked this button to do.

6. **Uniqueness is a property of the set, so it is fixed where the set is built.**
   `AudioLabel.ShortenAll(names)` shortens each name and then, for any group whose shortened form
   collides, returns the FULL name for every member of that group. `Shorten` keeps its per-name
   job. `EnumerateRenderEndpoints` uses the new one, which fixes Settings at the same time.

7. **No model-token heuristic.** The roadmap suggests the distinguishing token is the model
   number (`G733`, `PG27AQDM`) and that vendor names are noise. A rule that picks "the
   distinguishing token" out of an arbitrary device name cannot be right in general, and being
   wrong here means naming the device you are about to route your audio to incorrectly. Instead:
   the cell shows the disambiguated name, trimmed with an ellipsis if it must be, and **the
   tooltip and the accessible name carry the full name**, so nothing is lost to a hover or to a
   screen reader.

8. **A failed switch says so.** If `SetDefaultEndpoint` returns anything but `S_OK` or throws,
   the picker stays open and a line replaces the header. Silently doing nothing is the behaviour
   this product's comments argue against everywhere else.

9. **No absolute colours anywhere in the cells.** This is the specific defect that killed the
   System Controls page: its tiles were hard-coded blue-grey while the panel takes an accent
   tint, so on a lime accent they sat on green as cold blue rectangles. Every colour here comes
   from the notch palette, and `check-contrast.ps1` measures the pair.

## The design

### Layout

The frame is unchanged: `356x116`, 14 DIP of top padding, a 73 DIP content band, 29 below (20 of
it the page rail's lane).

```
 356 x 116, the picker mode
+--------------------------------------------------------+   14  top padding
|  < Output                                              |   14  header
|                                                        |    3  gap
|  +---------------------+  +---------------------+      |   16  row 1
|  | * Logitech G733 ... |  |   Realtek(R) Audio  |      |    4  gap
|  +---------------------+  +---------------------+      |   16  row 2
|  |   PG27AQDM (NVI...  |  |   Steam Streaming.. |      |    4  gap
|  +---------------------+  +---------------------+      |   16  row 3
|  |   Steam Streaming.. |  |                     |      |
+--------------------------------------------------------+
|                      -  ---  -                         |   20  page rail lane
+--------------------------------------------------------+
```

`14 + 3 + (16 * 3) + (4 * 2) = 73`. Each cell is `(320 - 10) / 2 = 155` DIP wide. The dot
marking the current default is 6 DIP, in the cell's leading edge.

16 DIP per row is the thinnest number in this design and it is a row rather than a card, so the
label sits on a 15 DIP line. **This is the number most likely to need correcting on a render**,
and correcting it means dropping the header to 12 or the gaps to 3; the frame does not move.

### The mode

`MediaWidget` gains two states and one rule: the tile row and the progress row are one visual
group, the picker grid is the other, and exactly one is visible. The output control switches to
the picker; the header's back control and the `Escape` key switch back. A track change while the
picker is open does NOT switch back: the person is doing something else, which is the same
principle `NotchEventPolicy` already applies to the whole frame.

### Switching the default

A new `Plith.Services.OutputDeviceSwitcher`:

```csharp
public static bool TrySetDefault(string endpointId)   // Console + Multimedia
```

It declares the undocumented interfaces with the GUIDs the roadmap already recorded: the class
`870af99c-171d-4f9e-af0d-e63df40c2bc9` and the interface `f8679f50-850a-41cf-9c72-430f290290c8`.
Only `SetDefaultEndpoint` is declared with a real signature; the methods before it in the vtable
are declared as reserved slots, because the vtable order is what makes the call land and an
interface that omits them would call the wrong function.

Returns false rather than throwing. The HRESULT is logged through `DiagnosticLog`, because a
COM interface with no contract behind it is exactly the thing whose failure has to be readable
later.

### Naming

```csharp
public static IReadOnlyList<string> AudioLabel.ShortenAll(IReadOnlyList<string> names)
```

Shorten each; group by the result; any group with more than one member gets its full names back.
`WindowsAudioClient.EnumerateRenderEndpoints` runs its names through it. `Shorten` itself does
not change, so the classic card and the audio rail keep behaving as they do.

### Accessibility

Every cell is a `Button`, so it has an automation peer, and its name is the FULL device name plus
its state: `"Logitech G733 Gaming Headset, current output"` or `"Realtek(R) Audio"`. Arrow keys
move between cells and `Enter` or `Space` activates, forwarded the way the shelf's grid does it.
The back control is a `Button` named "Back to now playing". The header's failure line is set on an
element that owns a peer, which `check-a11y.ps1` enforces.

## Testing

Unit tests, on pure code only:

- `AudioLabel.ShortenAll`: a colliding pair returns both full names; a non-colliding list is
  shortened as before; a three-way collision; an empty list; one name.
- The ordering rule: default first, then enumeration order, and the case where the default id is
  not in the list at all (a device removed between the two reads).
- `NotchGeometry.OutputPickerCapacity` equals its grid's product.

Checks and harnesses:

- `render-widgets.ps1` draws the picker in both themes, with the five-device fixture and with a
  seven-device one so the overflow cell is seen.
- `check-a11y.ps1` and `check-contrast.ps1` cover the new cells.
- `drive-media-page.ps1` gains a stage: open the picker, read the tree, press the second device,
  confirm through NAudio that the default endpoint actually changed, **and then switch it back**.
  A check that leaves someone's audio on a different device is a rude check.

## Risks and open items

1. **16 DIP rows.** The thinnest number here. Judge it on a render, not in this document.

2. **`IPolicyConfig` is undocumented and can break on any Windows release.** It is the only way
   in, Windows' own Sound panel uses it, and the failure mode is bounded: `TrySetDefault`
   returns false, the picker says so, and the overflow door to `ms-settings:sound` still works.

3. **A device list can change while the picker is open** (a headset sleeps, a monitor is
   unplugged). The grid is built when the picker opens and not refreshed while it is up, which
   is the shelf frame's own rule for the same reason. A press on a device that has since gone
   returns false and lands in the failure line.

## Out of scope, with reasons

- **Input devices.** The mic already has its own control on the clock page, and a picker that
  switched both would need two grids in a 73 DIP band.
- **The communications role**, per decision 5.
- **Per-application routing.** Windows has no API for it short of the audio session APIs, and it
  is a feature, not a detail of this one.
- **Voicemeeter-aware switching.** On a machine where Voicemeeter is the monitored source, the
  Windows default and what Plith displays are deliberately different things. This button changes
  the system's output and says so; making it also move Plith's monitored endpoint is a separate
  decision that was offered and not taken.
