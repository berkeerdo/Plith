# R7 part 2: deferred review minors

Collected 2026-11-26 from the plan's review files before its scratch folder was deleted. Text as
the reviewers wrote it (diff line numbers refer to the review packages, not the files). Some were
fixed later: the 0 ms exit recheck (Task 7) by `5625860`, the helper UIAccess check (Task 11
item 4) by `5cff893`, the final review's criticals and importants in the fix wave. Everything
else is open and low risk; pick items up when touching the file.

## Ledger lines (deferred and parked)

- Task 6: complete (commits 83d8310..e5de580, opus review clean). Parked minor: compare leaf encoded bytes with PLITH_CERT instead of cached SHA-1 (final wave).
- Task 7: minor (deferred): acl.rs read_ace no bounds check vs AceSize (malformed -> Unknown)
- Task 7: minor (deferred): stop_pid silently skips when OpenProcess TERMINATE denied
- Task 7: minor (deferred): 0 ms exit recheck after failed TerminateProcess
- Task 8: minor (deferred): uninstall closes processes before the audit runs (audit first would be cleaner)
- Task 8: minor (deferred): multi_sz decode strips two NULs unconditionally; round-trip test lacks trailing empty entry
- Task 8: minor (deferred): trust_helper leaves cert in stores when helper verification fails
- Task 8: minor (deferred): REG_MULTI_SZ without list terminator treated as malformed (fail-safe)
- Task 9: minor (deferred): OS code parsed from reason text; add os_code: Option<u32> to StepError
- Task 9: minor (deferred): worker opens the file at the exe path, not proven to be the running image (swap needs another Plith-signed file; worst case older signed setup as uninstaller)
- Task 10: minor (deferred): art decoded once at start scale (other scale class draws stretched 1x)
- Task 10: minor (deferred): unreadable Run key counts as none, update unticks silently; no test for Update on Fresh/OverWpf
- Task 10: minor (deferred): others listed in task-10-review.md
- Task 11: minor (deferred): seven minors in task-11-review.md
- Task 13: minor (deferred): unreadable config.ini skipped without a log line (+2 in task-13-review.md)
- Task 15: minor (deferred): download follows any HTTPS redirect (signature is the trust check)
- Task 15: minor (deferred): final-path refusal ends in Ready (endless re-download) instead of Failed; launch.rs doc says manifest asks UAC (it is asInvoker); \?\ stripped so >MAX_PATH fails safe
- Task 16: minor (deferred): remaining minors in task-16-review.md; translations of non-Turkish notice/rows unreviewed by a native speaker
- Final: parked - N1 relaunch loop with 'Run as administrator' compat flag - Ruling: real, deferred to before public release (marker argument on relaunch); not hit in Task 12 (no compat flag). Cost if wrong: a UAC loop for users with that flag.
- Final: parked - N2 shell accepts but SmartScreen blocks relaunch -> silent - Ruling: deferred, log only; cost: confusing first run on a blocked download.
- Final: parked - N3 uninstall window cwd = root\setup keeps folder until exit - Ruling: harmless, folder is delete-on-reboot anyway.

## task-1-review

- crates/plith/Cargo.toml:21: the `windows` feature list was left as is and may carry features
  now used only by `plith-host` (Globalization, Registry, UI_Shell, perhaps Com). Trim in a later task.
- docs/decisions/0021-own-setup-with-elevated-worker.md:22: `PLITH'İ AÇ` uses a straight
  apostrophe where the spec uses a typographic one; cosmetic.

## task-2-review

1. wire.rs:555-558: `u8::parse` accepts a leading `+` and leading zeros, so `S+1` and `S01` decode to Step(1). Encode never produces them, so the wire is not canonical. Fix: require all-ASCII-digit text without leading zeros (or length 1). Add a test for `S+1`.
2. args.rs:126: the pipe check is only `starts_with` the prefix. The suffix is unchecked, so it can contain `\` or `..`. It is harmless today because the worker takes no path from arguments, but restricting the suffix to `[0-9a-zA-Z-]` would be cheap hardening.
3. wire.rs:547-549: `encode` emits `Failed{step: 0 or 9}` and `Step(9)`, which `decode` then rejects. This is asymmetric and unguarded, and a debug_assert or a clamp would document it.
4. args.rs:140-143: `(` and `)` are quoted although Windows does not need it. The brief asked for this (test name), so it is fine. Newline and other control characters are not in the quote set, which is also fine for argv parsing.

## task-3-review

- pl.rs:524 "Instalacja nie ruszyła." is colloquial; "Instalacja się nie rozpoczęła." is more neutral.
- setup/tests.rs:616-622 `every_language_is_translated` checks only `LeadStart`; weak guard against an untranslated copy-paste in other strings (short captions such as START legitimately equal English in de/pl, so a blanket check needs an allow-list).
- `border` is not contrast-tested and `border`/`track` are identical (theme.rs:686,689); the implementer's own picks, fine if Task 4 renders look right.
- fit.rs lead/title styles (Body/Strong) are assumptions; Task 4 must keep them in sync.

## task-4-review

- Process: tests were written after the implementation, with no RED run (report "TDD evidence"). The brief's step 2 (FAIL first) was skipped. Honest, but it is a deviation.
- screen.rs:420-451 and :399-417: on Done the autostart target (y 356..384) overlaps the close-link target (y 382..406) by 2 DIP. The check wins because it comes first. Harmless; trim the link rect to cy-10 if you want them disjoint.
- others.rs:494-495: `Error { step: 0 }` clamps the background to `BG_ART[0]`, but `steps(Some(0))` draws no label. This is an unreachable edge today. Clamp `step` once and use it for both.
- tests.rs `every_screen_in_eight_languages_builds`: asserts only list size, non-empty text and target x-bounds. It does not check that text rects stay inside the window (long translations at LINE_W are covered only by the render fit test in another crate).
- tests.rs: no assertion on the lead and sub y positions (250, 262/292, 236/274), the step label places, or the close mark rect, so the README numbers are only partly pinned. The Newer and Uninstalled close pills and the Error retry rect are likewise not checked against (220, 298).
- Close-link and checkbox widths use the estimated 7.4 DIP per char (screen.rs:273-277). This is acknowledged in the report. Task 5 should confirm underline and target widths against measured text.

## task-5-review

- tests.rs:~536: assertion message "a ticked box has its mark" is stale; on now swaps a Stroke for a Fill, so the `+1` Fill count still holds but only by that swap. Assert on the stroke/fill kind instead.
- screen.rs `others.rs:452`: `UninstallDone` also uses Display (not asked); sensible, but tell Berke.
- 40 PNGs, ~35 MB of binaries committed; consider whether they belong in git.
- `NOCHMAL` is informal next to formal De strings elsewhere; `NEU VERSUCHEN` might fit if width allows.

## task-6-review

1. trust.rs:288-293: compare the leaf's encoded bytes (`pbCertEncoded`/`cbCertEncoded`) with
   `PLITH_CERT` instead of (or as well as) the cached `CERT_SHA1_HASH_PROP_ID`. The property can be
   set explicitly on a context and is persisted for store copies; byte equality with the embedded
   cert is the stronger pin and removes any reliance on the property cache. Low practical risk today
   (the leaf context comes from the signed message), but it is a one-line hardening in code that
   gates an elevated worker.
2. trust.rs:171: `code.is_ok()` accepts any non-negative HRESULT; WinVerifyTrust success is exactly
   0. Use `code == S_OK` (or `code.0 == 0`) so an unexpected positive return is not treated as valid.
3. trust.rs:195: `HSTRING::from(path.as_os_str())` keeps an interior NUL; WinTrust then checks the
   prefix before the NUL. Std file and process APIs reject such a path, so it is not exploitable
   through Rust callers, but rejecting it with `Os` is cheap and makes "the file verified is the
   file named" hold by construction.
4. trust.rs:171 with an untrusted root: the Authenticode chain policy reports one error, so other
   chain errors on the pinned leaf (expired without timestamp, wrong EKU, possibly a Plith cert put
   in `Disallowed` by an admin) may be masked by `CERT_E_UNTRUSTEDROOT`. Only Plith's own cert is
   affected, so not a bypass; worth a sentence in the module doc. Unverified which error Windows
   ranks first for untrusted root plus explicit distrust.
5. store.rs:422-440: on `LocalMachine` the system store `Root` is a collection that includes the
   GroupPolicy and Enterprise physical stores. A copy found there makes `has_cert` true (fine) but
   `remove_cert` fails with access denied (that physical store is read-only), which would fail the
   uninstall step. Consider treating a delete failure on a policy copy as non-fatal, or document it.
6. store.rs:401: `add_cert` takes any DER and can write any certificate into `LocalMachine\Root`.
   A `add_plith_cert(scope, store)` wrapper that always passes `PLITH_CERT` would make the elevated
   caller unable to install anything else by mistake.
7. tests.rs:639-654 depends on Defender binaries being present; the report already names this. A
   skip prints a note, so CI can go green without exercising `WrongSigner`.

## task-7-review

- `acl.rs:154`: only `ACCESS_ALLOWED_ACE_TYPE` (0) is read. Callback or conditional allow ACEs (types 9 and 11) and object allow ACEs (5) granting write pass silently. Fail closed on any allow-family type other than 0.
- `acl.rs:58-78`: the skip is fail-open on path form. `C:\PROGRA~1\Plith`, `\\?\C:\Program Files\Plith` or a mapped form skips the security check without notice. Root is internal (global constraints), so this is low risk. Logging "permission check skipped" when it does not apply would make it visible.
- `tests.rs:251-259`: `only_paths_under_program_files_are_acl_checked` hardcodes `C:\Program Files`, so it fails on a machine whose Program Files is elsewhere. Build the positive case from `SHGetKnownFolderPath`.
- `processes.rs:132-134`: a failed `CreateToolhelp32Snapshot` returns no PIDs, and `stop_by_path` reports success. Return an error instead.
- `processes.rs:98-125`: `SeDebugPrivilege` stays enabled for the rest of the worker. Disable it after the stop.
- `walk.rs:23`: `root` itself is not checked for being a reparse point before `read_dir(root)`. Its parent is admin-only in practice, but a one-line `symlink_metadata` guard is cheap defense in depth.
- `walk.rs:75-87`: the read-only fallback clears the attribute on a hard-linked file's shared record. Harmless (the name is deleted next), noted for completeness.
- `build.rs:55`: a missing resource compiler only warns, so a release payload build can ship without VERSIONINFO or icon. Panic when `PLITH_SETUP_PAYLOAD` is set.
- `files.rs:97-100`: a failed stage on a fresh root leaves the empty `osd`/`setup` folders it created. A rerun cleans them, cosmetic only.
- WPF's `Setup\` folder keeps its capital S when it survives as our `setup\` (case-insensitive merge). Cosmetic only.
- Tests: no file-symlink case (needs privilege, acceptable) and no test that a stop failure deletes the staged files (`files.rs:73-76`).

Checks run: read spec section 5 step 1 (`docs/specs/2026-11-25-r7-installer-design.md:105-112`) for the order; `plith_core::setup::is_ours` (`crates/plith-core/src/setup/files.rs:5-11`) for the case and separator handling the walk relies on; `Get-AclProblems` and its constants (`scripts/install-osd.ps1:35-65`) for the port; `crates/plith-osd/build.rs` for the manifest pattern. No tests run.

## task-8-review

- `uninstall.rs:1052-1056`: processes are closed/killed before `audit_root`, so an unsafe root refuses after the user's Plith was already stopped. Install audits first (`files.rs:85`); the controller note said "audit first". Reorder (audit when root exists, then close).
- `uninstall.rs:1091-1099`: if `image` were not under `root` (only possible through the test seam, `running_image` guarantees otherwise) the loop would schedule every ancestor directory; add a `starts_with(root)` guard.
- `running_image` (`uninstall.rs:1113`) has no test (canonical path/case logic is the core of the "except running" claim); the seam tests inject `image` directly. A test with a differently cased root string would pin it.
- `trust_helper` leaves the certificate in the stores when verification fails (reported; matches spec order, fine).

## task-9-review

1. A same-user process can connect to the window's pipe before the worker (the DACL grants the user full access, `security.rs:69`); the worker then gets `ERROR_PIPE_BUSY` and runs silent, and the impostor can feed `Finished` to the window. Task 10 must treat the worker's exit code as the truth and reports as progress only; optionally the server checks `GetNamedPipeClientProcessId` equals the worker pid from `start_worker`.
2. `WriteFile` on the client is synchronous (`pipe.rs:646`): a server that never reads stalls the worker mid-install while it holds the mutex. Only reachable by whoever chose the pipe (after Critical 1 is fixed, only a same-session window name), so low risk; a bounded write (writer thread with a timeout, then drop the client) would remove it.
3. `start_worker` passes no `lpDirectory` and no `hwnd` (`elevate.rs:278-286`): the elevated process may start with the caller's working directory and the UAC prompt can open behind the window. Set `lpDirectory` to System32 and pass the window handle in Task 10.
4. `os_code` (`worker.rs:930-952`): the concern in the report is valid but not a security issue. It parses free text, so a file name containing `(0x...)` could leak a wrong code if a format ever puts the path after the error; non-Win32 HRESULTs come out as large decimals (`0x80004005` to `2147500037`). The cost of a wrong value is one wrong number in a localized line. Prefer `os_code: Option<u32>` on `StepError` set where the `io::Error` or `windows::core::Error` is converted; fine as a follow-up.
5. `Global\Plith.Setup.Worker` can be pre-created by a medium process to force exit 3 (`worker.rs:815`); denial only, already noted in the report.
6. `worker_refuses_at_medium` asserts a marker in a temp dir the worker would never touch (`worker/tests.rs:974-985`); that part proves nothing (plan-mandated). The exit code and no-report assertions are the real checks.
7. `#[allow(dead_code)]` on whole modules `pipe` and `elevate` (`main.rs:360-369`) also hides dead code in the parts the worker uses; narrow it to the items Task 10 needs.
8. TDD RED was not captured (report, "TDD evidence").

## task-10-review

1. `flow.rs:405-408`: `NewerInstalled` is matched before `Mode::Uninstall`, so an explicit `--uninstall` of a setup copy older than the installed entry shows the "newer installed" screen and cannot uninstall. Normally own == installed so it is rare; consider letting Uninstall win.
2. `flow.rs:513-521`: a non-numeric `Failed` reason is dropped (code 0) and the log line holds only `step N failed, code 0`; the worker's reason text never reaches `setup.log`, contrary to "Log detail only in setup.log". Log the raw reason (it is already length-bounded by the wire format).
3. `flow.rs:542`: `on_elevate_failed` is not guarded by `running()`; a stale `ElevateFailed` would overwrite Done/Error. Not reachable today (one pump at a time), cheap to guard.
4. `flow.rs:506`: `Report::Step(done)` is not monotonic or clamped; an out-of-order lower Step moves the ripples backwards. The pipe is ordered, so low risk.
5. `effects.rs:1562` (diff line): `remove_dir_all` error is discarded after the log was closed, so a failed data delete is invisible. Reopen or buffer one line, or report it on the Uninstalled screen later.
6. `effects.rs:1519`: if `self.window` were None, the flow is already Running with no worker (stuck, Close ignored). Unreachable after `open`, but call `on_elevate_failed(false)` there for safety.
7. Close/Esc/Alt+F4 are all ignored while Running (binding decision), so a hung worker leaves an unclosable window; accepted by the decision, noting it.
8. `app.rs:1222`: `CreateMutexW` failure returns None and exits 0 silently; log it.
9. Report: "Update and repair also tick autostart" is listed as a concern, not a blocker; it is the Important finding above.

## task-11-review

1. build-setup.ps1:130 vs :187: the "never overwrite" check runs at the start, but `Copy-Item` runs minutes later and overwrites silently if the file appeared meanwhile. Re-test `Test-Path` just before the copy, or copy to a temp name in `release\` and `Move-Item` without `-Force`.
2. build-setup.ps1:187: the published copy is not re-verified. Run `Get-AuthenticodeSignature` on `$output` after the copy, or compare its hash with `$setup`. The "Setup" row (:192) then reports a file that was proven good.
3. signing.ps1:213-216 / build-setup.ps1:118: with two valid `Plith Self-Signed` certs the newest wins and the script fails on the thumbprint although the pinned one exists. Prefer selecting the pinned thumbprint from `Cert:\CurrentUser\My` when given.
4. The osd UIAccess request is not verified in the output (for example a byte search of `plith-osd.exe` for `uiAccess="true"` in the embedded manifest). If the env var did not reach the build script, a non-UIAccess helper would be signed and shipped silently.
5. build-setup.ps1:136-141 and :161-166: `Remove-Item Env:` removes the variable even when the caller had it set before. Save and restore the previous value if that matters.
6. No `#Requires -Version 7`. Under Windows PowerShell 5.1 with `$ErrorActionPreference='Stop'`, cargo's stderr output would turn into terminating errors. build-osd.ps1 shared this before, and the run used `pwsh`.
7. build-setup.ps1:123: with `Stop`, a bad `cargo metadata` output fails inside `ConvertFrom-Json` with a JSON error before the clear message at :124. Capture the output to a variable first, check `$LASTEXITCODE`, then convert.

## task-13-review

- settings_file.rs:246: `.ok()?` swallows any config.ini read error other than absence (for example a non-UTF-8 ANSI file with a Turkish city name). The import is silently skipped and the defaults are written, with no log line. Consider logging a non-NotFound error (`import: cannot read config.ini: ...`).
- wpf.rs:73: `Auto` after an earlier `Dark` resets to the default (last wins). This is consistent, but untested.
- Core `wpf` tests were written with the implementation, not RED first (the report admits it).

## task-14-review

- Report admits tests and code were written in one pass with no recorded RED run; process note only.
- settings.rs KEYS and the test key list reflowed to one key per line (rustfmt width), noisy diff but harmless.

## task-15-review

- crates/plith/src/update/launch.rs:954-961: `hwnd` is null and the thread has no COM init; `ShellExecuteExW` documents that COM should be initialised on the calling thread, and a windowless elevation request from a background thread can come up as a flashing taskbar button instead of a prompt. Check in Task 17; `CoInitializeEx(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)` on the update thread is the documented pattern.
- crates/plith/src/update/tests.rs:1530-1540: `disabled_check_never_fetches_but_check_now_does` asserts `fetches == 0` immediately after `enable(false)`, which is trivially true before the thread even runs; it does not show that a due timer is skipped while disabled. Correct by construction (service.rs:1183 uses `recv()`), but the test proves less than its name. A pure `next_wait(enabled, ...) -> Option<Duration>` helper would be testable directly.
- crates/plith/src/app/state/update.rs:599-623: no test for `drain_update` persistence (Checked/Seen reach the saver, unchanged values do not resave). The report says the state test file is near its cap; a sibling `state/update/tests.rs` would fit.
- crates/plith/src/update/service.rs:1191-1195: `InstallReady` with no ready release does nothing and emits no row, so a click that races a restart is silently dropped. Emit `Row(Failed)` or `Row(Current)` so the UI never shows a stuck button.
- crates/plith-host/src/http/download.rs:270-273: redirect host not restricted (brief deviation, see Spec Compliance). Document the decision in the brief or the ADR.
- crates/plith/src/main.rs:695-699: module-wide `dead_code` allow; remove in Task 16.
- Process: the report states tests were not written RED first (task-15-report.md:28-31). No failing run was captured.

## task-16-review

1. `crates/plith-shell/src/tray.rs:1608-1615` (`notice`): when `added` is false (Explorer not up yet at sign in) nothing is shown, but `TrayHost::show_notice` returns true and `update.rs` logs `update: notice shown for <v>`. The log lies and the notice is lost with `Seen` recorded. Make `notice` return bool, log `not shown` otherwise.
2. `notify.rs:1786-1791` (`copy_units`): a cut can split a surrogate pair at the buffer end (emoji in a future title); strip a trailing lone high surrogate. Not reachable with current copy.
3. `notify.rs:1778`: `icon(hwnd)` loads the small-icon size (SM_CXSMICON) but the flag is `NIIF_LARGE_ICON`; Windows upscales, a bit soft. Load the large size for the balloon icon.
4. `update.rs` `on_tray_notice`/`install_update`: a balloon click while a download runs (About button hidden then) queues a second `InstallReady`. Guard on the row being `Ready`.
5. `STATE_W` duplicated (`sheet/about.rs` and `lang/sheet/fit.rs`); make one `pub(crate)` const.
6. TDD evidence: implementer admits the core tests were not seen red, app/shell tests written after code. Process note only.

## final-review

- M1. flow.rs:79: `--update` with nothing installed shows the update screen and, being
  `Mode::Update`, does not tick autostart; spec section 5 says "a normal install". Map
  `(Mode::Update, Fresh)` to `Start`. Untested today.
- M2. uninstall.rs:62-67: the Apps entry is removed before the certificates; if a store write
  fails the user has no entry left to retry from (only a downloaded setup with `--uninstall`).
  Remove the certificates first, the entry last.
- M3. flow.rs:66-69 with effects.rs:65-76: on update, repair and uninstall a `Plith` Run value
  that belongs to another program (`Some(false)`) is deleted by `SetAutostart(false)`. Delete
  only when the value is ours.
- M4. Running the installed `setup\plith-setup.exe` in install mode fails at the last rename
  (its own image is mapped, files.rs:207-224) after Plith was closed; app and helper are already
  new. Skip the setup copy when its bytes are equal, or show a clear line.
- M5. elevate.rs:112-121: no `hwnd` on the `runas` call, so the UAC prompt can come up as a
  flashing taskbar button behind the window (Task 9 review Minor 3). Pass the window handle
  (the pump has it). Worth doing before Task 12 so the real run is not misread.
- M6. state/update.rs:82-95 with service.rs:113: `Seen` is persisted even when the balloon was
  not sent (no tray icon yet at sign-in); the notice for that version is lost.
- M7. Duplication: three `REPARSE_POINT` constants (audit.rs, walk.rs, service.rs), two
  `SHGetKnownFolderPath` copies (worker.rs:52, register.rs:47, plus acl.rs:73), `"plith.exe"` in
  effects.rs:17 next to `INSTALLED_FILES[0]`, own version parsed in three places, and two
  unrelated `is_ours` in plith-core (setup files vs Run value). A `plith_host::known_folder`
  helper and one const would do.
- M8. docs/plans/active/r7-installer.md: no checkbox is ticked although Tasks 1-11 and 13-16 are
  done (project-docs rule: tick as work progresses). The ledger has the truth; tick from it in
  Task 17 with the honest banner.
- M9. Task 12 facts to plan around: settings.ini already exists here, so the WPF import will not
  run in Task 12 (prove it separately, e.g. with Berke's yes on a renamed settings.ini, or record
  that only unit tests cover it); the install will `WM_CLOSE` Berke's running dev Plith
  (`target\release\plith.exe`, pid 50480); the current Run value points at the WPF Debug build
  and will be rewritten to `C:\Program Files\Plith\plith.exe`.
