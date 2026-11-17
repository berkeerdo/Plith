# 0013 Windows outputs get their own view, not a one-route diagram

Date: 2026-11-17. Status: accepted. Decided by the master; Berke handed the design over
("yaratıcılığı sana bıraktım, nasıl olmasını istiyorsan öyle yap").

## Context
Without Voicemeeter the audio page drew the track diagram with one source ("Windows") and one
route to the default endpoint. It read as "Logitech G733 62" with no label, unit or bar
(Berke: "anlaşılmıyor hiç ne olduğunu"), and the footer opened with "VOICEMEETER ÇALIŞMIYOR",
treating the common case as an error.

## Decision
Without Voicemeeter the audio page is an outputs view in the grammar of the system page's
brightness row, not a diagram:
- **Default output** on top: device glyph, name (Name style), caption `VARSAYILAN · <KIND>`,
  the brightness ruler's drawing (ticks, thumb, shared helpers) on its own x range, 264 to
  `PAGE_W - 90`, so the caption clears the first tick and `SESSİZ` clears the ruler end (the
  first render collided at brightness's 200 to `PAGE_W - 70`), with the filled part and thumb in `Flow` (sound
  is flowing, the colour of a live route), and the value as `%62` (Turkish puts the sign
  first). Clicking the ruler sets the volume (`Action::VolumeRuler`, like brightness). Muted:
  filled part and thumb `Faint`, value `SESSİZ`.
- **Other outputs** below a faint `DİĞER ÇIKIŞLAR` label: glyph, name (Body, muted ink),
  kind caption, a hollow station at the ruler start. Clicking the row makes it the default.
  They show no volume: only the attached endpoint's volume is known and live.
- **Glyphs** are 16 DIP drawings from display-list strokes with small filled parts (ear cups,
  speaker cone) so they hold weight next to the name (headphones, speaker, display),
  chosen by `DeviceKind`: Windows form factor first (headphones, headset, display interface),
  then name keywords (headset, headphone, kulaklık, buds), else speaker.
- Footer: `ÇIKIŞA TIKLA: VARSAYILAN OLUR`. Voicemeeter is not mentioned anywhere.
- Brightness shows `%70` too, so the two rulers read alike.
- The Voicemeeter view (routes, buses, dB) is unchanged.

## Consequences
- `plith-audio` reads `PKEY_AudioEndpoint_FormFactor` per endpoint; core maps it to
  `DeviceKind` (pure, tested). Core stays free of Windows types.
- Volume of non-default outputs stays unknown; showing it needs per-endpoint volume callbacks,
  a later service change if ever wanted.
- `AudioFacts.attached` carries no endpoint id; the default row trusts that the service attaches
  to the default. True today (nothing calls `set_pin`); a pin would show the pinned endpoint's
  volume on the default row. Carry the id before a pin is ever offered.
- Setting volume from the island is a hardware write: verified on this PC only with Berke's yes.
