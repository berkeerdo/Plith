# Plith identity package: directions

Date: 2026-11-24. Skill `praxvon-direction`. Brief: a new logo and app icon, the whole settings
window, the installer screens, as one package (Berke: "logo iconunu ve komple ayarları
güncelleriz", "setup tasarımı da değişmeli"). Must sit on the existing island language: Ray
Paneli (ADR 0007, `specs/2026-11-14-r3-visual-direction.md`), four themes, upper-case mono labels,
the lime (flow) colour only for "sound flows here now".

## 1. Product truth
- Plith is a strip of instrument at the top of a Windows screen for people who care about sound
  and play games. The island is the instrument; settings and install are what sits behind it.
- People open settings once or twice and then forget them; install happens once, behind a UAC
  prompt that asks them to trust a signed helper. Both are moments of trust, not of daily use.
- The icon lives at 16 px in the tray and taskbar and at 256 px in the installer and Explorer.
- The one claim: Plith is precise and does nothing behind your back.

## 2. The attractor (banned as a core idea)
- Icon: a rounded pill or capsule on a dark rounded tile, maybe a dot or a wave in it; a notch cut
  out of a screen outline.
- Settings: a Windows 11 Settings clone (sidebar with icons, cards with toggle switches, Mica,
  accent colour) or macOS preference panes; a live preview of the island at the top; theme cards
  with thumbnails (already banned in the settings spec).
- Installer: a Next, Next, Finish wizard with a hero render of the notch on a desktop, a progress
  bar and a "Launch Plith" checkbox.
- From `fingerprints.md`: lime on near-black with upper-case mono and `[BRACKETS]` and `+` corners
  taken with no source (Teenage Engineering / Nothing lineage); bento grid; glass cards.

## 3. Ten directions (drawn domains: stained glass, space mission control, abacus and counting
boards, Islamic geometric tiling)
| # | Domain | Reference | Idea | p |
|---|---|---|---|---|
| 1 | stained glass | Chartres lead cames | sections as leaded panes, lead = track | 0.06 |
| 2 | stained glass | glazier's cutline (full-size cartoon, every piece numbered with its glass code) | settings as a cutline; a theme is a glass code | 0.04 |
| 3 | mission control | Apollo MOCR go/no-go poll | each section answers GO; install is the launch poll | 0.08 |
| 4 | mission control | NASA Flight Rules (numbered rule, its rationale) | every setting is a rule with its "because" | 0.05 |
| 5 | mission control | Apollo DSKY verb/noun | settings typed as verb and noun | 0.12, dropped |
| 6 | counting boards | Nuremberg Rechentisch (jetons on ruled lines) | each setting a line, the value is where the jeton lies | 0.04 |
| 7 | counting boards | soroban (heaven bead on or off) | toggles as heaven beads | 0.09 |
| 8 | tiling | Darb-i Imam girih tiles | icon from girih strapwork | 0.07 |
| 9 | tiling | Fez zellij cutting template | pieces cut from one glazed square | 0.05 |
| 10 | free | Severn Bridge Junction box, Shrewsbury: mechanical lever frame, 1903, with interlocking | settings as numbered levers that lock each other | 0.06 |

Dropped: 5 (above 0.10). 8 drifts to ornament on a 16 px icon. Survivors developed: 10, 6, 4.

## 4. Three concepts

### A. Kol Sırası (lever frame)
| Field | Content |
|---|---|
| Source | The lever frame of Severn Bridge Junction signal box (Shrewsbury, 1903): a row of numbered levers with brass plates, and a locking frame under the floor that stops a lever being pulled while a conflicting one is. It is the room under the York NX panel that Ray Paneli already draws. |
| Metaphor | The settings window is the lever frame below the island's panel. |
| The rule | A setting shows what it depends on: a lever that cannot be pulled is drawn locked, with the lever that holds it named on its plate. |
| First screen | One frame of levers per section, read left to right: each lever is a numbered stem (normal = upright, pulled = leaning toward you, filled) with its plate above it (name, one line of what it does). The sidebar is the box diagram: sections as lever groups 1-10, 11-20. |
| Signature interaction | Pull `EKRAN GÖSTERGESİ` back to normal and lever 14 `WINDOWS GÖSTERGESİNİ GİZLE` drops a lock bar; its plate says `14 KİLİTLİ: 13 ÇEKİLİ DEĞİL`. That is the real rule in the app today (the flyout row is disabled while the OSD is off), finally made visible. |
| Type and colour | The island's mono for plate numbers and words, the sans for the plate's one line; ink only, the theme's floor; no lime in settings (nothing flows there), the pulled lever is filled ink. |
| Motion and sound | The lever swings with one detent (the engine's spring, short), the lock bar slides; nothing else moves. |
| Icon | The island silhouette at the top edge of a screen frame, and under it one lever at the pulled angle: reads as "notch" at 16 px, the lever is the Plith detail at 32 px and up. |
| Installer | Three levers: `1 KUR`, `2 WINDOWS İLE BAŞLAT`, `3 YARDIMCIYA GÜVEN` (the signed OSD helper); lever 3 names what UAC will ask before it asks. Pull 1 to install; the frame shows each lever's state as the setup runs. |
| Swap test | No: the lever frame is the sibling of Plith's own NX panel and its interlocking is Plith's own setting dependency; on another app it would be a costume. |
| Build reality | Direct2D display list like the current window, same text stack; levers are paths. Cost: one session for the window, one for the installer screens. Risk: a lever row holds fewer words than a list; long translations need the plate's second line. Keyboard: levers are focusable, Space pulls. |

### B. Jeton Tahtası (counting board)
| Field | Content |
|---|---|
| Source | The Nuremberg Rechentisch: a table ruled with lines, where brass jetons (Rechenpfennige) lie on or between the lines and the whole account is read at a glance. |
| Metaphor | Settings are one account laid out on one board. |
| The rule | Nothing is behind a page: one board, every setting a ruled line, its value is where the jeton lies. |
| First screen | No sidebar. Four columns of ruled lines (Genel, Görünüm, Boşta, OSD), each line a setting with its choices as positions, a jeton on the chosen one. |
| Signature interaction | Click a position and the jeton slides along the line and clicks onto it. |
| Type and colour | Mono for line names, the jeton is ink on the theme floor. |
| Icon | The island pill as a jeton lying on one ruled line. |
| Installer | Casting the account: three lines, jetons laid as each step completes. |
| Swap test | Weak: a board of lines could be any app's settings. |
| Build reality | Simple to build; risk: one board with every setting is wide, the city field and long language list strain it. |

### C. Uçuş Kuralları (flight rules)
| Field | Content |
|---|---|
| Source | NASA Flight Rules: each rule numbered by section, the rule in one line, then its rationale. |
| Metaphor | Settings are the mission rules the island flies by. |
| The rule | No setting without its "because": every row states what it changes and why you would. |
| First screen | A ruled binder page: `RULE 3-12 EKRAN GÖSTERGESİ`, the choice, then a dim `NEDEN:` line. |
| Signature interaction | Changing a rule stamps its revision letter and time (`REV B 16:42`). |
| Type and colour | Mono rule numbers, sans rationale. |
| Icon | The island with a rule number tick under it. |
| Installer | Launch commit criteria: each item turns `GO`. |
| Swap test | Middling: the binder voice fits many technical apps. |
| Build reality | Easy; risk: more text, less instrument. |

## 5. Recommendation
**A, Kol Sırası.** It is the room under the panel Plith already draws, so icon, settings and
installer extend Ray Paneli instead of replacing it, and its interlocking is not decoration: it
shows a dependency the app really has. From C, keep one idea: each lever plate carries the one
line of what it does.
