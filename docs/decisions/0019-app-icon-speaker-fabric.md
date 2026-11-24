# 0019 The app icon is a lime speaker-fabric tile with a black source

Date: 2026-11-24. Status: accepted (Berke picked the direction and said "önerini uygula
sıkıntı yok devam edelim"). Replaces the 0.4.0 icon in `crates/plith/assets/plith.ico`.

**Context:** identity round 1 (lever frame, code-drawn pixel icons) was rejected as crude.
Round 2 made the logo its own job: a board of the category's real icons, then about 60
image-model drafts shown on pages Berke could open (`design/identity/icon/README.md`). Every
notch app on the board uses a black pill, purple glass or a mascot face. Berke rejected the P
monogram, picked "one dark mark on a lime tile", then the tactile fabric look. Two vector
redraws were rejected: three black arcs on green read as Spotify, and fabric drawn in code
looked like graph paper.

**Decision:** the icon is a rounded lime (`#CAFF33`) tile of speaker fabric, a glossy black
half-disc source set into its top edge (where the island lives), and three concentric ripples
pressed into the fabric, tone on tone, ending inside the tile. No black stripes. The master is
the image-model drawing `design/identity/icon/plith-master-1024.png` (transparent background);
`scripts/build-icon.ps1` crops it to the tile and writes 16 to 256 px PNG entries into the
`.ico`. The tray, the exe and Explorer all read that file.

**Consequences:**
- Lime now also means the brand on the icon, not only "sound flows here now" on the island.
  The island's rule is unchanged.
- Berke: the fabric does not read at small sizes ("kumaş kısmı anlaşılmıyor"); at 16 to 32 px
  the icon reads as a lime tile with a black top and ripples. A hand-fitted small master is the
  fix if the tray size needs it; a first vector attempt was heavier than the downsample.
- The master is a raster, so the 256 px entry is the largest sharp size. A store or installer
  hero larger than 1024 needs a new drawing.
- Settings and installer screens derive from this icon next (identity package, R7 part 2).
