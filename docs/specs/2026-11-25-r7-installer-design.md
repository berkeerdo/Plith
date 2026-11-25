# R7 parity part 2: installer, uninstall, WPF settings import

Date: 2026-11-25. Status: approved by Berke 2026-11-25 ("önerinle devam edelim"); approach picked ("önerin ne ise devam edelim"); builder check done.
Paths are under `docs/` unless they start with `crates/` or `scripts/`. Inputs: research
`research/2026-11-24-r7-parity.md` section 3, screens `specs/2026-11-24-identity-r3-design.md`
(direction A Halka) and `design/identity/r3/installer/README.md` (assets, layout in DIP), update
check `specs/2026-11-25-update-check-design.md` (approved, built last in this part), ADR 0002
and 0009 (helper trust), part 1 plan `plans/active/r7-parity-tray.md` (tray, version 0.4.0,
icon, Run key module).

## 1. Intent
Today the only way to install Plith is the WPF 0.3.x installer, so WPF cannot be removed. Part 2
gives the Rust app its own install, uninstall and upgrade path, carries a WPF user's few
settings that still mean something, and lets an installed Plith learn about the next version.
The installer is the first thing a new user sees, so it is drawn in the icon's material (A Halka)
and must feel as made as the island.

## 2. Goals, non-goals, anti-goals
Goals
- `Plith-Setup-<version>.exe`: one signed file that installs, upgrades (also over WPF 0.3.x),
  updates and uninstalls Plith, in the three A Halka screens plus an error screen.
- One UAC prompt per install, named on screen before it comes.
- After install: `C:\Program Files\Plith\plith.exe`, `C:\Program Files\Plith\osd\plith-osd.exe`
  (path stays as hardcoded in `crates/plith-ipc/src/launch.rs`), the helper's UIAccess works on a
  clean machine, Start Menu entry, Apps list entry with icon and version, optional start with
  Windows, Plith running at medium integrity.
- Uninstall from the Apps list removes everything it installed and asks before touching user data.
- First Rust start after a WPF install keeps the WPF user's theme, weather on or off, and city.
- The update check from its spec, ending in this installer's update screen.

Non-goals
- A real code-signing certificate, MSIX, winget (ROADMAP items, unchanged).
- Silent or unattended switches, repair, modify, per-user install, a choice of install folder.
- Brightness hotkeys and endpoint pinning: Berke's `config.ini` shows neither was ever set
  (`BrightnessEnabled = False`, hotkeys 0, `MonitoredWindowsEndpointId` empty). Recorded as
  dropped by design, can come back on request. Summon hotkey likewise.
- Deleting the WPF sources from the repo (the cleanup commit after the parity check).

Anti-goals
- The elevated process acting on anything a medium process can write: no paths, file names or
  payload taken from arguments, the pipe or the user's folders. It installs only the bytes
  embedded in its own signed image.
- A progress bar or any comp invention (identity spec section 2); text baked into images.
- Plith started elevated, ever.
- Touching `%LOCALAPPDATA%\Plith` without asking; overwriting a Rust setting with a WPF one.
- A half install that a second run of the same setup cannot repair.

## 3. Constraints
- UIAccess needs the helper signed by a chain the machine trusts and in a secure folder (ADR 0002,
  0009). With the self-signed `CN=Plith Self-Signed` certificate (thumbprint
  `144A7919A1FC941FDC3A9443C1DBAA7AE002AC0F`, pinned in `scripts/install-osd.ps1`) that means
  adding it to `LocalMachine\Root` and `LocalMachine\TrustedPublisher`.
- The Run key is per user (`HKCU`). With over-the-shoulder elevation the elevated process runs as
  another account, so `HKCU` writes belong to the medium process.
- Pure logic in `plith-core` with unit tests; Windows code outside it. Structure limits (500
  source, 150 entry, 80 per function), no `mod.rs`. Every string in eight languages (ADR 0015).
- Builds through `heavy-run.mjs`. The private key stays in `CurrentUser\My`; no script creates a
  certificate (the `build-osd.ps1` policy; `sign-plith.ps1` is WPF and is not used).
- This PC has WPF 0.3.3 installed (`C:\Program Files\Plith`, Apps entry `Plith`, version
  `0.3.3+5e2feb9`) and a WPF `config.ini` next to a Rust `settings.ini`: the real upgrade case.
  Any install, upgrade or uninstall run here needs Berke's separate yes and his click on UAC.

## 4. Approaches (picked in conversation)
1. **One Rust setup exe drawn with our engine; the window runs at medium and starts itself
   elevated for the work (picked).** Fits the approved flow (the running screen names UAC before
   it comes), `PLITH’İ AÇ` starts Plith at medium with no trick, update and uninstall are modes
   of the same file. Costs a small progress channel between two processes.
2. One Rust setup exe that is `requireAdministrator` (as WPF): simpler, but UAC comes before the
   start screen and starting Plith at medium needs the `explorer.exe` trick.
3. Inno Setup: least code, but cannot draw A Halka; it would throw away the approved design.

## 5. Decided
### Shape
- New binary crate `crates/plith-setup` (`plith-setup.exe`, `asInvoker`, per-monitor DPI v2,
  icon and VERSIONINFO like `plith.exe`). Released as `release/Plith-Setup-<version>.exe`; the
  asset name stays so WPF's own update button finds it.
- Payload embedded at build time: the signed `plith.exe`, the signed UIAccess `plith-osd.exe`, the
  public certificate, the eight installer backgrounds and the button (1x, 2x). The setup reads
  its version from its own VERSIONINFO; the workspace version is the single source (0.4.0).
- Modes by argument: none = install or upgrade; `--update --from <ver>` = update screen
  (`0.4.0 → 0.5.0`, `GÜNCELLE`); `--uninstall`; `--work <mode>` = the elevated worker. `<ver>` is
  shown only, never used for a path.
- The worker is started with `ShellExecuteExW` verb `runas` on the setup's own path. It checks it
  runs elevated and that its own image is signed by the pinned certificate, then does the steps
  and reports each one to the window. The window draws the ripple of each finished step pressed.
- Medium side does: the screens, the Run key (`HKCU\...\Run` value `Plith`, the existing
  `plith::autostart` logic), starting `plith.exe` from `PLITH’İ AÇ`, uninstall's user-data choice.

- Signature check (my call, after the builder check): "signed by the Plith certificate" means a
  valid Authenticode signature whose signer thumbprint equals the pinned one; an untrusted root is
  accepted only for that thumbprint, since step 2 is what adds the trust on a clean machine. The
  same check guards the update download.
- Versions (my call): installed newer than the setup: a screen saying so with `KAPAT`, no
  downgrade. Equal: the start screen, and a run repairs. `--update` with nothing installed: a
  normal install. The installed version is read from the Apps entry, or `0.3.x` for WPF.
- Accounts (my call): this part supports elevation as the same user (Berke's case). Elevation
  with another admin's credentials is left for the standard-user measurement before release and
  written down as untested. The worker sends its log lines to the window, which writes
  `setup.log` in the user's own folder.
- Audience and the root certificate (my call): unchanged from WPF 0.3.x, which already put the
  same certificate into `Root` for its GitHub users. Known limit, stated in the release notes; a
  real certificate stays the ROADMAP fix.

### Install steps (worker), idempotent, in ripple order
1. `DOSYALAR` (my call, ordered so a failure never leaves the user with nothing): write every new
   file as `.new` beside its target first; only then close a running Plith (`WM_CLOSE` to
   `Plith.Notch`, wait; the helper exits when its client is gone, `plith-osd/src/watch.rs`; then
   by exact path, also another user's) and a running WPF `Plith.exe` by exact path, rename the
   `.new` files into place, copy the setup itself to `Program Files\Plith\setup\plith-setup.exe`,
   and last remove every other file under `Program Files\Plith` that is not ours (WPF exe, dlls,
   `runtimes\`, `Setup\`, DropCatcher). Assert the folder is writable only by SYSTEM,
   Administrators, TrustedInstaller. No full rollback.
2. `YARDIMCI`: add the certificate to `LocalMachine\Root` and `TrustedPublisher` if missing
   (match by thumbprint), verify `plith-osd.exe` with WinVerifyTrust.
3. `BAŞLAT`: Start Menu shortcut for all users (replaces WPF's), Apps entry under the same key
   `HKLM\...\Uninstall\Plith` (DisplayName `Plith`, DisplayVersion, Publisher `Plith`,
   DisplayIcon `plith.exe`, InstallLocation, UninstallString `"...\setup\plith-setup.exe"
   --uninstall`, EstimatedSize, NoModify, NoRepair).
- Done screen: `WINDOWS İLE BAŞLAT` ticked if a `Plith` Run value already exists (WPF's
  `AutoStart = True` left one), else ticked by default; `PLITH’İ AÇ`, `KAPAT`.
- Error screen: the failed step's name and one line of reason, `TEKRAR DENE`, `KAPAT`; the log
  goes to `%LOCALAPPDATA%\Plith\setup.log`. Rerunning the setup repairs a partial install.

### Uninstall
- Short A Halka flow in reverse (ripples rise): confirm screen with `KULLANICI VERİLERİNİ DE SİL`
  (unticked: settings, shelf, plays, logs stay), one UAC.
- Worker: close Plith and the helper, remove files, Start Menu entry, Apps entry, and the
  certificate from both stores (my call: nothing else should trust it once Plith is gone; on the
  dev PC a later `install-osd.ps1` then needs the trust re-added, noted in its script).
- The running setup copy cannot delete itself (my call): `MoveFileEx` delete-on-reboot for the
  `setup\` folder; a later install clears a leftover. Medium side removes the current user's Run
  value and, if ticked, `%LOCALAPPDATA%\Plith`; other accounts' Run values are left (a missing
  exe in Run is harmless).

### Screens without a comp (my call)
- Error, uninstall confirm, uninstall running (the four backgrounds in reverse), "newer version
  installed" and the update screen have no comp. I draw them from the A1 to A3 rules (same
  layers, same type, no new material) and Berke sees renders before the plan's build steps for
  them count as done.

### WPF settings import
- In `plith.exe` at start, only when `settings.ini` does not exist and `config.ini` does (so it
  runs once, and never overrides a Rust choice). Reads `[General] Theme` (`Dark` to `gece`,
  `Light` to `kagit`, `Auto` to the default), `[Osd] ShowWeather` to `weather`,
  `[Osd] WeatherLocation` to `city` (non-empty only, then `location=city`). Writes `settings.ini`
  and logs one `import:` line. `config.ini` is left in place. The parse and mapping live in
  `plith-core`.
- `AutoStart` travels as the Run value (see done screen); the shelf needs nothing (`shelf.txt` is
  already the WPF format).

### Update check
- As its spec, with one change: the installer is not `requireAdministrator`; `GÜNCELLE` asks UAC
  once through the worker. `KUR` in the app verifies the download's signer, then starts the setup
  with `--update --from <ver>`; the setup closes the running Plith in its first step.

### Build and release
- `scripts/build-setup.ps1`: through `heavy-run.mjs`, release build of `plith`, `plith-osd` with
  `PLITH_OSD_UIACCESS=1`, sign both (SHA256, timestamp), then build `plith-setup` with the signed
  files' paths in environment variables for `include_bytes!`, sign it, write
  `release/Plith-Setup-<version>.exe`, print each SHA-256.

## 6. Left to the builder
- The progress channel between window and worker (named pipe with a DACL for the user and
  Administrators, or the worker's exit code per step), as long as the worker reads nothing from it.
- Drawing: our D2D pieces from `plith-render` or a small renderer in the setup; the 2x assets
  scaled for other scales; a short press animation per ripple is allowed if measured.
- Installer language: from the Windows UI language, `lang::setup` table in `plith-core`.
- Exact text sizes and positions per `design/identity/r3/installer/README.md` and comps A1 to A3.
- How a running WPF `Plith.exe` (UIAccess, High) is closed: Restart Manager, then terminate by
  exact path with the elevated worker.

## 7. Done when
- Unit tests for the import mapping, version compare, step state and file plan; the five checks
  green (`cargo fmt --check`, clippy `-D warnings`, tests, `structure-check.mjs`, release build).
- Screens rendered at 1x and 2x in Turkish and one other language, seen by Berke.
- On this PC, with his yes: upgrade over WPF 0.3.3 (WPF files gone, Apps entry 0.4.0, helper
  runs with UIAccess, Plith at medium in the log, Run value points at the new path), a second run
  repairs, uninstall leaves no files, entry, shortcut or certificate, and reinstall works.
- Owed before release, not in this part: the standard-user helper measurement (STATUS risk).
