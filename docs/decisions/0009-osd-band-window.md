# 0009 The OSD helper draws a DComp swap chain in a window created in the UIAccess band

Date: 2026-11-16. Status: accepted on the signed spike (RDP, facts only); the visual check over
games is R6 Task 9. Drafted by lane `osd`, filed by the master at the R6 merge (`4aca03e`).

**Decision:** `plith-osd.exe` creates its window with the undocumented
`CreateWindowInBand(.., ZBID_UIACCESS = 2)`, resolved at run time, and draws with the
`plith-render` `Gpu` and `CompositionTarget` (DComp, flip sequential, latency 2, as ADR 0005 and
0008). Without UIAccess (unsigned dev runs) it falls back to a plain topmost window. The layered
window fallback of spec section 10 is not built.

**Measured:** signed and installed, `CreateWindowInBand` reads back band 2 with uiaccess=true and
the swap chain presents; `SetWindowBand` on an existing window is refused (Access denied) even
with UIAccess. The helper runs at High integrity; the medium notch writes to its pipe through an
explicit medium label and verifies the server's user and integrity level.

**Consequences:** motion lines go to `%LOCALAPPDATA%\Plith\plith-osd.log`, not `plith.log`.
Visibility over games, elevated windows and true exclusive fullscreen is recorded by the R6
console run; a true exclusive failure is a limit of composition, not of this path.
