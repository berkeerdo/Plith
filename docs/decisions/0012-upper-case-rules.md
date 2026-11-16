# 0012 Upper case: Turkish rules for our copy, invariant for foreign names

Date: 2026-11-16. Status: accepted. Rule from Berke on 2026-11-16. Drafted by lane `pages` (P7),
filed by the master at the R4 merge (`b6bd801`).

**Decision:** our own Turkish UI strings drawn upper-case use Turkish rules through
`plith_core::pages::copy::tr_upper` (i to İ, ı to I), so `SİSTEM`, `PARLAKLIK`. Names that come
from Windows, Voicemeeter or apps (devices, endpoints, strips, sources, titles) are never
upper-cased with Turkish rules: the renderer's style upper-casing is Rust's invariant
`to_uppercase`, so "Display" becomes "DISPLAY", never "DİSPLAY".

**Consequences:**
- Own strings in an upper-cased style (Label, Sub) are constants in `copy.rs`, each tested to
  equal its own `tr_upper`.
- Own copy in mixed-case styles ("Çalan bir şey yok", "Parlaklık", "Mikrofon", "Dosyaları buraya
  sürükle", "Kaynak N", "Kullanılmıyor") stays in its page module in Turkish sentence case and is
  never upper-cased. A test scans every fixture page.
- Foreign text reaches the renderer unchanged.
- `plith-osd` still has its own `upper_tr`; switching it to `tr_upper` is a wiring-job item.
