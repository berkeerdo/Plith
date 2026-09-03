# Phase 5 — Manual Verification

Phase 5 landed on `feature/phase-5-cardhost` in 27 commits. Every task passed an
automated review, the branch passed a whole-branch review, and the suites are green
(`Plith.Tests` 107/107, `Plith.Installer.Tests` 17/17, build at its 3 pre-existing
CA1861 warnings, `scripts/check-a11y.ps1` exit 0).

**None of that verifies the things below.** No test in this repository can observe a
rendered pixel, a UI Automation property, a focus adorner, or the Win32 fullscreen
detection path. These checks are the evidence for Phase 5's actual claims, and they
need a person.

Full per-task detail lives in the task reports under
`.superpowers/sdd/2026-09-02-phase-5-cardhost/` — but that directory is git-ignored
scratch that `git clean -fdx` will destroy, which is why this summary is in the repo.

---

## 0. Before you start

Close any running `Plith.exe` from the tray (**Exit**, not a force-kill — the app's
`OnExit` path unhooks `WH_KEYBOARD_LL` and flushes settings). Then build and run the
branch build, or you will be testing old code.

---

## 1. The pixel contract — the headline claim

Phase 5's central promise is that **the user sees no change**. The OSD stopped being
hardcoded markup and started rendering through `CardHost`, and the whole refactor is
only correct if the result is pixel-identical to 0.1.5.

**The reference screenshots were never captured.** That is a gap in how this was run:
the baseline step was reassigned from the agents to a human and then not requested at
the right moment. Two options:

- **Capture a true baseline.** `git worktree add ../plith-baseline be990a3`, build and
  run it, screenshot the OSD in both states, then remove the worktree. This gives a
  pixel-exact comparison.
- **Check descriptively.** The criteria below are specific enough to catch the failure
  class that actually occurred on this branch.

| # | Check | Pass | Fail |
|---|---|---|---|
| 1.1 | Volume-only OSD (no media playing) | No gap above the volume row; card width and corner radius unchanged | A visible 1 px line above the volume row, or any vertical shift |
| 1.2 | Volume + media OSD | Media row on top, **one** 1 px divider with 14 px above and below, volume row underneath | Divider missing, doubled, or different spacing |
| 1.3 | **Volume bar width** | The bar spans the card's full inner width and its fill tracks the level proportionally | The bar collapses to roughly its content width |

**1.3 is the highest-value single check.** Card content now sits inside two new wrapper
layers (an item `ContentPresenter` and a `ContentControl`). Static reasoning says
content still stretches to the full ~400 px, but that is reasoning, not measurement. If
it is wrong the failure is glaring.

| # | Check | Pass | Fail |
|---|---|---|---|
| 1.4 | **Live transition**: with the OSD on screen, start playback, then stop it | Media row and its divider appear together and vanish together; the card recentres without drifting sideways | The divider survives without a row above it, or the card jumps |

1.4 matters because a defect of exactly this class was found and fixed mid-branch: the
divider trigger was collapsing on *every* card, costing 29 px of separation. The static
markup is now correct; 1.4 is what confirms the runtime re-indexing is too.

---

## 2. Games must keep the OSD — the safety property

The app draws above fullscreen games, and Phase 5 added a feature that hides it during
fullscreen *video*. A false positive here silently destroys the headline feature.

**Run this with Windows Fullscreen Optimizations at their default (on).** Windows 11
converts most "exclusive fullscreen" games into borderless flip-model windows that
report `QUNS_BUSY` rather than `QUNS_RUNNING_D3D_FULL_SCREEN` — so for those games the
D3D veto never fires and safety rests entirely on the media-session check. Forcing a
game into true exclusive mode tests only the easy path.

| # | Check | Pass | Fail |
|---|---|---|---|
| 2.1 | Launch a borderless-windowed game. Start Spotify or a YouTube tab **playing** in the background. Focus the game. Press a volume key. | The OSD appears | The OSD does not appear |
| 2.2 | With the game still focused, check `plith.log` | No `Suppression -> True` line while the game holds focus | That line appears — a failure **even if the OSD looked fine** |

2.1's background-media requirement is not optional. With no media playing the check
short-circuits before the risky comparison and passes vacuously.

| # | Check | Pass | Fail |
|---|---|---|---|
| 2.3 | Fullscreen Netflix or VLC, playing. Press a volume key. | No OSD | The OSD appears |
| 2.4 | Exit fullscreen | The OSD works again | Still suppressed |
| 2.5 | **Hover test**: during 2.3, with Hover keep-alive enabled, move the cursor over where the OSD would be | Nothing appears | The OSD resurrects |
| 2.6 | Turn the Settings toggle off, repeat 2.3 | The OSD appears during fullscreen video | Still suppressed |

2.5 checks a guard added in the final fix wave. `OnMouseEnter` used to restore the OSD
without consulting the suppressor; whether that was ever reachable depends on layered-
window hit-testing, which nobody verified.

---

## 3. Screen reader (Narrator: `Ctrl+Win+Enter`)

**3.1 and 3.2 are done.** They were settled by reading the live UI Automation tree —
the same data a screen reader consumes — rather than by starting Narrator, which has the
advantage that the result is a transcript instead of something someone has to remember
hearing. 3.3 still needs a person, because the Settings window can only be opened from the
tray menu.

**The open question behind this whole section is answered: the band window DOES
participate in the UI Automation tree.** The doubt recorded here earlier — that a
`CreateWindowInBand` HWND might be invisible to UIA, which would have excused an
announcement failure — is disproven. It appears as a desktop child with its full contents
below it. So an a11y property that does not show up is a defect, not a platform limit.

Two real defects were found that way and are fixed on this branch:

- **The live region never existed.** Both card views put
  `AutomationProperties.Name`/`LiveSetting` on a bare `<Grid>`. WPF creates no automation
  peer for a panel, and `UIElementAutomationPeer.GetNameCore` reads the property off its
  owner — so the name reached nothing. The tree showed `Custom <AudioCardView> name=''`
  with no element for the Grid at all, not even in the raw view. The properties now sit on
  each `UserControl` root, which does own a peer, and `LiveRegionAnnouncer` raises
  `LiveRegionChanged` on change (WPF does not raise it by itself).
- **Cards announced their .NET type name.** The OSD's `ItemsControl` container takes its
  name from the bound item, falling back to `ToString()`. The tree really did read
  `DataItem name='Plith.Cards.AudioCard'`. `ICard.AccessibleName` now requires every card
  to name itself, and each card's `ToString()` returns it.

Measured after the fix, with a media session active:

```
DataItem name='Now playing'
  Custom <MediaCardView> name='OK (feat. Don Toliver) by Kanye West, paused'
    Button name='Previous track'   Button name='Play'   Button name='Next track'
DataItem name='Volume'
  Custom <AudioCardView> name='Remote Audio, 60%'
```

| # | Check | Status |
|---|---|---|
| 3.1 | Transport buttons named | **PASS** — "Previous track" / "Play" / "Next track", each with HelpText |
| 3.2 | Volume value reaches the live region | **PASS after fix** — the card's name tracks the current value |
| 3.3 | Tab through the whole Settings window | **Still open** — needs the window opened from the tray |

Note that 3.2 is verified only as far as *the property is correct and the event is
raised*. Whether Narrator actually speaks it is a Narrator behaviour question that only
listening can settle; if you ever do run Narrator, that is the thing worth checking.

`scripts/check-a11y.ps1` passed throughout, including while the live region was inert: it
only asked whether interactive controls were named, never whether a name could reach UI
Automation. It now also fails on AutomationProperties set on a peerless element, and that
second check was validated in both directions — green on the fixed tree, and four findings
when pointed at the tree from before the fix.

Two known cosmetic quirks to listen for and report — both are deferred, and whether
they are worth fixing depends on whether you can actually hear them:

- Muting may be announced twice ("MUTED" then "muted"), because `GainText` is set
  before `Muted` in `AudioCardViewModel.Apply`. A one-line reorder fixes it.
- A track may briefly announce as `" by , playing"` if the media session appears before
  its metadata arrives.

---

## 4. Keyboard and focus (Settings window)

Nine accent swatches became keyboard-reachable in Phase 5 — they were previously
`Border` elements with mouse handlers, unreachable by keyboard and invisible to screen
readers.

| # | Check | Pass | Fail |
|---|---|---|---|
| 4.1 | Tab from the title bar to the bottom | Every interactive control reachable, in visual order | Anything skipped or out of order |
| 4.2 | Watch the focus indicator throughout | A visible accent ring on every focused control, **including the accent swatches** | Any control focuses with no visible indicator |
| 4.3 | Focus a swatch, press **Space**, then **Enter** | Both select that accent | Either does nothing |

4.3's Enter half would have failed as written. `ButtonBase.OnKeyDown` in the WPF source
handles `Key.Enter` only when `KeyboardNavigation.AcceptsReturn` is set, and it defaults to
false — so a focused, non-default `Button` ignores Enter, whatever a `Click` handler
suggests. The swatch code carried a comment claiming Space and Enter both worked. The
swatches now set `AcceptsReturn`, which is safe here because Settings declares no
`IsDefault` or `IsCancel` button for Enter to reach instead. Space was always fine.

| 4.4 | Repeat 4.2 in both Light and Dark themes | Ring legible in both | Invisible in either |

**On mouse clicks:** if the ring appears on click as well as on Tab, check whether
Windows' "show keyboard cues" setting is on. With it enabled that is correct WPF
behaviour, not a defect.

Tabbing now costs nine presses to cross the accent row — a known ergonomic wart,
deferred as a Phase 6 item.

---

## 5. High contrast (`Left Alt + Left Shift + Print Screen`)

The static audit is done; nobody has looked at the result. **This section resisted
automation twice over, so it stays a human step:**

- Setting the `HCF_HIGHCONTRASTON` flag through `SystemParametersInfo` flips
  `SystemParameters.HighContrast`, so Plith swaps to its HighContrast palette — but
  Windows 11 applies the actual contrast theme through the theme engine, so `SystemColors`
  still hands back ordinary theme colours. The palette would be exercised against the wrong
  input. Use the real hotkey or Settings > Accessibility > Contrast themes.
- Over an RDP session the OSD cannot be screenshotted at all. The band window is layered
  and drawn with `UpdateLayeredWindow`; it is absent from a plain `BitBlt`, absent from one
  with `CAPTUREBLT`, and `PrintWindow` with `PW_RENDERFULLCONTENT` returns solid black. The
  window is provably on screen and correctly positioned at the time — only its pixels are
  unreachable. Run anything pixel-based from the physical console.

| # | Check | Pass | Fail |
|---|---|---|---|
| 5.1 | Toggle high contrast on with the OSD visible | Palette swaps live, no restart | Stale colours |
| 5.2 | OSD appearance | System colours; card border and divider clearly visible | Border or divider invisible |
| 5.3 | Volume bar | A single system colour | Still green/amber/red |
| 5.4 | No accent tint anywhere | Card and bar use system colours only | The picked accent still tints them |
| 5.5 | Settings window | All text legible | Anything unreadable |
| 5.6 | **Hover and press controls** | Feedback still perceptible | States indistinguishable |
| 5.7 | Toggle high contrast off | The accent theme returns | Stuck |

5.6 is a deliberately unresolved judgment call: hover and pressed brushes all collapse
onto `ControlColor` because high contrast offers nothing more granular. Only looking at
it settles whether that is acceptable.

Known limitation, already documented in the palette files: switching *between* high
contrast themes (Black → White) leaves stale colours until restart. Toggling high
contrast on and off works correctly.

---

## 6. Regressions the refactor could have caused

| # | Check | Pass | Fail |
|---|---|---|---|
| 6.1 | Start or quit Voicemeeter without touching the volume | The OSD does **not** appear on its own | It pops by itself — the baseline reset is not reaching `AudioCard` |
| 6.2 | Media transport buttons | Each controls playback, and the OSD stays alive across all three clicks | A button does nothing, or the OSD fades mid-sequence |
| 6.3 | No media session | Volume row only, no blank gap where the media card would be | A gap or stray divider |
| 6.4 | Compact mode on, then off | Media row hides and returns; the Settings preview agrees with the real OSD | They disagree, or toggling pops the OSD |
| 6.5 | Summon hotkey and volume keys | Both pop the OSD; no Windows native flyout alongside | Either fails |
| 6.6 | Position edit mode | Enter, drag, save, cancel all work | Any step broken |
| 6.7 | Clear the fullscreen hide list in Settings, restart | It stays empty | The defaults come back |

6.1 and 6.3 are the two that validate specific refactor decisions — baseline
suppression moving into `AudioCard`, and removing the `HasSession` visibility binding
from the media view's root.

---

## Reporting back

Note the check number and what you saw. A failure in section 1 or 2 is worth stopping
for; sections 3–5 are quality gates whose failures are worth recording even when they
do not block.
