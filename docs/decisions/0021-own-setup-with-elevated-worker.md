# 0021 Plith installs with its own drawn setup that elevates a worker

Date: 2026-11-25. Status: accepted (picked in conversation, R7 part 2). Design:
`docs/specs/2026-11-25-r7-installer-design.md`, section 4.

**Context:** the installer must carry the approved A Halka design, name UAC on the running
screen before it comes, start Plith at medium integrity from `PLITH'İ AÇ`, and also serve update
and uninstall. Three ways were weighed:
1. One Rust setup exe drawn with our engine; the window runs at medium and starts itself elevated
   for the work.
2. One Rust setup exe that is `requireAdministrator` (as the WPF setup): simpler, but UAC comes
   before the start screen and starting Plith at medium needs the `explorer.exe` trick.
3. Inno Setup: least code, but it cannot draw A Halka and would throw away the approved design.

**Decision:** approach 1. `plith-setup.exe` is `asInvoker`, draws its own screens, and starts a
worker with `ShellExecuteExW` verb `runas` on its own path (`--work <mode>`). The worker checks
that it runs elevated and that its image is signed by the pinned certificate, does the steps and
reports each one to the window over a small progress channel. Update (`--update --from <ver>`)
and uninstall (`--uninstall`) are modes of the same file. The Windows helpers it shares with
`plith.exe` (log file, display language, Run key, WinHTTP GET) live in `plith-host`.

**Consequences:**
- UAC appears only after the person pressed the install button, so the running screen can say it
  is coming, and `PLITH'İ AÇ` starts Plith at medium with no trick.
- Costs a progress channel between two processes and a signature check on the worker's own image
  and on the update download.
- The setup drawing code is ours to maintain; there is no installer framework to lean on.
- Elevation as another admin account is not covered in this part; it is measured before release.
- Addendum (final review): a setup window that was itself started elevated (WPF's update button
  uses `runas`, or "Run as administrator") starts itself again through the desktop shell
  (`IShellWindows` desktop view, `IShellDispatch2::ShellExecute`), which runs at medium, and
  exits. If that fails, or the shell is elevated too, the window stays but never starts Plith.
