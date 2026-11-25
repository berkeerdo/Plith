# R7 parity part 2: installer, import, update check Implementation Plan

> **Finished 2026-11-26, merge waiting on Berke.** A ticked box means the step was taken, not
> that nothing deviated. Deviations, rulings and decisions are in the progress log below, in
> `docs/STATUS.md`, `journal/2026-11.md` and `quality/2026-11-25-setup-first-run.md`. Boxes of
> Tasks 1 to 11 and 13 to 16 were ticked at the close from the task reviews, not as each task
> finished. Task 5 Step 3: Berke delegated the look ("Bakmadım, sen karar ver", ruling R14).
> Task 17 Step 2: the scheduled check measured the request, `DENETLE` was not clicked.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Plith-Setup-0.4.0.exe` installs, upgrades (also over WPF 0.3.x), repairs and uninstalls
Plith in the A Halka screens; a first Rust start keeps a WPF user's theme and weather; an
installed Plith learns about a new version once a day and installs it on a click.

**Architecture:** Pure logic (versions, situations, screens, wire messages, WPF import, update
rules) in `plith-core`; Windows helpers shared by `plith` and the setup in a new crate
`plith-host`; the new bin crate `plith-setup` runs its window at medium and starts itself with
`runas` as a worker that installs only the bytes embedded in its own signed image and reports
over a named pipe. Screens are `DisplayList`s painted by the existing `SheetRender`.

**Tech Stack:** Rust 1.98 edition 2024, `windows` 0.62.2 (new features `Win32_Security_WinTrust`,
`Win32_Security_Cryptography`, `Win32_System_Diagnostics_ToolHelp`), D2D through `plith-render`,
WinHTTP, `winresource` =0.1.34, PowerShell build script, signtool.

**Spec:** `docs/specs/2026-11-25-r7-installer-design.md`; update check
`docs/specs/2026-11-25-update-check-design.md`; screens `docs/design/identity/r3/installer/README.md`.
Interface map used to write this plan: section numbers below refer to its findings, all checked
against the code on 2026-11-25.

## Global Constraints
- Branch `feature/r7-installer`. Ask Berke before the first commit on it (one yes for the branch);
  merge to `main` and every push asked separately.
- Limits: 500 source, 150 entry (`main.rs` composes only), 800 test, 80 per function; `src/foo.rs`
  + `src/foo/bar.rs`, never `mod.rs`. Count every touched file before editing (sizes below).
- `plith-core` stays Windows-free with no dependencies; tests beside code as `foo/tests.rs`.
- Every user string in eight languages (`Lang::ALL`, ADR 0015); Turkish captions upper-case with
  `lang::upper`. No em dash anywhere.
- Install root `C:\Program Files\Plith`; helper `osd\plith-osd.exe` (const `INSTALLED_HELPER`,
  `crates/plith-ipc/src/launch.rs:17`); setup copy `setup\plith-setup.exe`.
- Pinned thumbprint `144A7919A1FC941FDC3A9443C1DBAA7AE002AC0F`. Apps key
  `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith`. Start Menu link
  `%ProgramData%\Microsoft\Windows\Start Menu\Programs\Plith.lnk` (same name WPF used).
- Run value: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` `Plith` (existing
  `plith_core::tray::run_value`), written only by the medium process.
- The worker never takes a path, file name or payload from arguments, the pipe or user folders.
- Builds and full test runs: `node ~/.claude/bin/heavy-run.mjs -- <cmd>`. Five checks before
  merge: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`,
  `node scripts/structure-check.mjs`, `cargo build --release --workspace`.
- Any install, upgrade or uninstall on this PC: Berke's separate yes, he clicks UAC.

## Review Focus
1. UAC cancelled (`ShellExecuteExW` fails with `ERROR_CANCELLED`): back to the start screen with
   one line, not the error screen. Test in Task 9.
2. A file in the install root is locked (WPF `Plith.exe` survived the close, an Explorer preview):
   step 1 fails naming the file, no `.new` file left beside a target, rerun succeeds. Test in Task 7.
3. Two setups at once (double click twice): the second raises the first (`Local\Plith.Setup`
   mutex). Test in Task 10.
4. `--work` started by hand at medium, or from an unsigned build: refuses with one log line and
   exit code 2, touches nothing. Test in Task 9.
5. Setup started from `Downloads\Plith-Setup-0.4.0 (1).exe`: the `runas` command line and the
   copy to `setup\` survive spaces and parentheses. Test in Task 9 (argument quoting).

---

### Task 1: `plith-host` crate (move only) and the ADR

Move `crates/plith/src/log.rs` (156 lines), `locale.rs` (82), `autostart.rs` (218) with their
`tests.rs`, and `crates/plith-weather/src/http.rs` (157) into a new crate; move
`crates/plith-core/src/weather/json.rs` to `crates/plith-core/src/json.rs` with
`pub use crate::json` kept in `weather.rs`. No behavior change.

**Files:**
- Create: `crates/plith-host/Cargo.toml`, `src/lib.rs`, `src/log.rs`, `src/locale.rs`,
  `src/autostart.rs`, `src/http.rs` (+ their `tests.rs`), `README.md`
- Create: `docs/decisions/0021-own-setup-with-elevated-worker.md` (spec section 4, approach 1)
- Modify: `Cargo.toml` (`plith-host` path dep), `crates/plith/Cargo.toml`,
  `crates/plith/src/main.rs` (drop the three `mod` lines), callers' `use` paths,
  `crates/plith-weather/Cargo.toml` and its `http` callers

**Interfaces:**
- Produces: `plith_host::log::{data_dir, LogFile, now, format_stamp, Stamp}`,
  `plith_host::locale::windows_language() -> String`,
  `plith_host::autostart::{Autostart, RunKey, RegistryAutostart}`,
  `plith_host::http::get(url: &str) -> Result<String, String>` (now `pub`),
  `plith_core::json::{parse, Json}`.

- [x] Step 1: move files with `git mv`, fix `use` paths; `RunKey::delete` etc. unchanged.
- [x] Step 2: run `node ~/.claude/bin/heavy-run.mjs -- cargo test --workspace`; expected: the same
      test count as on `main` before the move, all pass (write the count in the progress log).
- [x] Step 3: clippy and `node scripts/structure-check.mjs` pass.
- [x] Step 4: commit `refactor(host): shared windows helpers crate`.

### Task 2: setup logic in `plith-core`

**Files:**
- Create: `crates/plith-core/src/setup.rs`, `setup/version.rs`, `setup/situation.rs`,
  `setup/files.rs`, `setup/wire.rs`, `setup/args.rs`, `setup/tests.rs`
- Modify: `crates/plith-core/src/lib.rs` (36 lines, `pub mod setup;`)

**Interfaces:**
- Produces:
  - `pub struct Version { major: u32, minor: u32, patch: u32 }` with `Ord`, `Display`
    (`0.4.0`); `Version::parse(&str) -> Option<Version>` accepts `0.4.0`, `v0.4.0`,
    `0.3.3+5e2feb9...` (build metadata ignored), rejects `0.4`, `abc`.
  - `pub enum Situation { Fresh, OverWpf { from: Version }, Upgrade { from: Version }, Repair,
    NewerInstalled { installed: Version } }`;
    `pub fn situation(own: Version, apps_entry: Option<&str>, has_wpf_exe: bool) -> Situation`
    (`apps_entry` is the `DisplayVersion` text; WPF if `has_wpf_exe` or major.minor < 0.4).
  - `pub fn is_ours(relative: &str) -> bool`: true only for `plith.exe`, `osd\plith-osd.exe`,
    `setup\plith-setup.exe` (case-insensitive, `/` or `\`); everything else under the root is
    removed by step 1.
  - `pub enum Report { Step(u8), Log(String), Failed { step: u8, reason: String }, Finished }`;
    `pub fn encode(&Report) -> Vec<u8>`, `pub fn decode(&[u8]) -> Option<Report>`; one message
    per pipe write, `reason` and `Log` capped at 200 bytes, step in 1..=3.
  - `pub enum Mode { Install, Update { from: Version }, Uninstall, Work { job: Job, pipe: String } }`,
    `pub enum Job { Install, Uninstall }`; `pub fn parse_args(args: &[String]) -> Mode`
    (unknown args give `Install`; `--work` without a valid job or pipe name gives `Install`);
    `pub fn quote_arg(&str) -> String` (Windows command-line rules).

- [x] Step 1: tests in `setup/tests.rs`: `version_orders_and_parses` (`0.3.3+5e2feb9` <
  `0.4.0` < `0.10.0`; `0.4` is None), `situation_over_wpf_on_this_pc` (`situation(0.4.0,
  Some("0.3.3+5e2feb9"), true)` is `OverWpf{from:0.3.3}`), `situation_newer_installed`,
  `situation_repair_on_equal`, `situation_fresh_without_entry`, `is_ours_keeps_only_three_files`
  (`Plith.dll`, `runtimes\win\x.dll`, `Setup\Plith-Uninstaller.exe` false), `wire_round_trips`
  and `wire_rejects_garbage_and_step_out_of_range`, `args_work_needs_job_and_pipe`,
  `quote_arg_handles_spaces_parens_and_quotes` (`a b (1)` and `a"b`).
- [x] Step 2: run `cargo test -p plith-core setup`; expected FAIL (module missing).
- [x] Step 3: implement the signatures above.
- [x] Step 4: run again; expected PASS.
- [x] Step 5: commit `feat(core): setup versions, situations, wire`.

### Task 3: setup strings and palette

**Files:**
- Create: `crates/plith-core/src/lang/setup.rs`, `lang/setup/{en,tr,de,es,fr,pt,it,pl}.rs`,
  `lang/setup/tests.rs`
- Modify: `crates/plith-core/src/lang.rs` (325, `pub mod setup;`), `crates/plith-core/src/theme.rs`
  (168, `Palette::setup()`)

**Interfaces:**
- Produces: `pub enum SetupText` with `ALL`, `pub fn setup_text(t: SetupText, lang: Lang) ->
  &'static str`, `SetupText::is_caption`. Variants: `Version` (`{v}` placeholder not used: version
  drawn as its own text), `Step1` `DOSYALAR`, `Step2` `YARDIMCI`, `Step3` `BAŞLAT`, `LeadStart`,
  `Install` `KUR`, `LeadRunning`, `SubUac` (names the coming Windows prompt), `DoneTitle`,
  `DoneSub`, `Open` `PLITH’İ AÇ`, `Autostart` `WINDOWS İLE BAŞLAT`, `Close` `KAPAT`, `Update`
  `GÜNCELLE`, `UpdateLead`, `Retry` `TEKRAR DENE`, `ErrorTitle`, `NewerTitle`, `UninstallLead`,
  `Uninstall` `KALDIR`, `DeleteData` `KULLANICI VERİLERİNİ DE SİL`, `UninstallDone`,
  `UacCancelled`. Turkish copy for the leads and subs is written in this task and shown to
  Berke with the renders in Task 5.
- `Palette::setup()`: `floor` lime as sampled from `bg-0-1x.png` (fallback), `ink` near black,
  `muted` dark olive, `flow` lime (button label on the black pill), `faint` for the step labels.

- [x] Step 1: tests: `every_setup_string_in_eight_languages` (non-empty, captions upper-case under
  `lang::upper`), `step_labels_fit` (each `Step*` measured by the existing fit helper fits 140 DIP
  at the `Label` style: reuse `lang/sheet/fit.rs` approach).
- [x] Step 2: run, expect FAIL; Step 3: implement; Step 4: run, expect PASS.
- [x] Step 5: commit `feat(lang): setup strings in eight languages`.

### Task 4: setup screens as display lists

**Files:**
- Create: `crates/plith-core/src/setup/screen.rs`, `setup/screen/{start,running,done,others}.rs`,
  `setup/screen/tests.rs`

**Interfaces:**
- Consumes: `SetupText`, `Palette::setup`, `Node::Image`, `ArtId`, `DisplayList`.
- Produces:
  - `pub const SETUP_W: f64 = 640.0; pub const SETUP_H: f64 = 420.0;`
  - `pub const BG_ART: [ArtId; 4]`, `pub const BUTTON_ART: ArtId`.
  - `pub enum Screen { Start, Running { done: u8 }, Done { autostart: bool }, Error { step: u8,
    reason: String }, Newer { installed: Version }, Update { from: Version, to: Version },
    UninstallConfirm { delete_data: bool }, Uninstalling { done: u8 }, Uninstalled,
    UacCancelled }`
  - `pub enum SetupAction { Install, Open, Close, ToggleAutostart, ToggleDeleteData, Retry,
    Uninstall, Drag }`; `pub struct SetupTarget { rect: Rect, action: SetupAction }`
  - `pub fn setup_build(screen: &Screen, own: Version, lang: Lang) -> (DisplayList,
    Vec<SetupTarget>)`; `pub fn setup_hit(targets: &[SetupTarget], x: f64, y: f64) ->
    Option<SetupAction>`.
- Positions (DIP, README): version label (18, 14); close 16 from the right; step labels 1 at
  (430, 38), 2 at (468, 68), 3 at (496, 108); start lead centred at y 250, button 200 x 55 at
  (220, 298); running lead y 262, sub y 292; done title y 236 (26 px, `Heading`), sub y 274,
  button (220, 304), autostart y 370, close y 394. Background: `BG_ART[done]` full window;
  uninstall uses the same four in reverse (`3 - done`). The rest of the window above y 215 is
  drag area (`SetupAction::Drag`).

- [x] Step 1: tests: `start_has_button_at_readme_rect`, `running_draws_background_for_done_steps`
  (done 2 gives `BG_ART[2]`), `done_targets_open_autostart_close`, `error_names_step_and_retry`,
  `uninstall_backgrounds_run_in_reverse`, `drag_area_above_ripples`, `every_screen_in_eight_languages_builds`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(core): setup screens`.

### Task 5: screen renders for Berke (gate for the screens without a comp)

**Files:**
- Create: `crates/plith-render/examples/render_setup.rs` (copy `render_sheet.rs`'s
  `write_png` and a PNG decode into `ArtPixels` with ids from Task 4)

- [x] Step 1: run `cargo run --release -p plith-render --example render_setup --
  docs/design/identity/r3/setup-renders` (new folder): every `Screen` in `Tr` and `En` at 1x and
  2x, names `<screen>-<lang>-<scale>x.png`.
- [x] Step 2: compare start, running, done against comps A1 to A3 side by side; fix offsets.
- [x] Step 3: show Berke all screens (private page or the folder); his words go into the progress
  log. Error, newer, update and uninstall screens count as done only after his yes.
- [x] Step 4: commit `docs: setup screen renders`.

### Task 6: signature and certificate stores in `plith-host`

**Files:**
- Create: `crates/plith-host/src/trust.rs`, `trust/store.rs`, `trust/tests.rs`
- Modify: `crates/plith-host/Cargo.toml` (WinTrust, Cryptography features)

**Interfaces:**
- Produces:
  - `pub const PLITH_THUMBPRINT: &str = "144A7919A1FC941FDC3A9443C1DBAA7AE002AC0F";`
  - `pub enum SignError { NotSigned, BadSignature, WrongSigner(String), Os(String) }`
  - `pub fn verify_plith(path: &Path) -> Result<(), SignError>`: `WinVerifyTrust` with
    `WINTRUST_ACTION_GENERIC_VERIFY_V2`, revocation off; `Ok` on success or on
    `CERT_E_UNTRUSTEDROOT`/`CERT_E_CHAINING` only when the signer thumbprint (SHA-1 of the leaf,
    from `WTHelperProvDataFromStateData` and `CertGetCertificateContextProperty
    CERT_SHA1_HASH_PROP_ID`) equals `PLITH_THUMBPRINT`; any other signer is `WrongSigner`.
  - `pub enum Store { Root, TrustedPublisher }`, `pub enum Scope { LocalMachine,
    CurrentUserTest(&'static str) }`; `pub fn has_cert(scope, store, thumbprint) -> Result<bool,
    String>`, `pub fn add_cert(scope, store, der: &[u8]) -> Result<(), String>` (idempotent),
    `pub fn remove_cert(scope, store, thumbprint) -> Result<(), String>` (missing is Ok).
- [x] Step 1: tests: `unsigned_file_is_not_signed` (a temp copy of a fresh file),
  `installed_helper_is_plith_signed` (runs only if `C:\Program Files\Plith\osd\plith-osd.exe`
  exists, else returns early with a printed note), `store_add_has_remove_round_trip` on
  `Scope::CurrentUserTest("PlithTest")` with the public cert DER from Task 7's assets, store
  deleted at the end.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(host): plith signature and cert stores`.

### Task 7: `plith-setup` crate, payload and the file step

**Files:**
- Create: `crates/plith-setup/Cargo.toml`, `build.rs`, `plith-setup.manifest` (asInvoker,
  PerMonitorV2, longPathAware, like `plith-osd.manifest`), `src/main.rs` (compose only),
  `src/payload.rs`, `src/files.rs`, `src/files/tests.rs`, `src/processes.rs`
- Create: `crates/plith-setup/assets/` (copy the 10 installer PNGs, `plith.ico`, and
  `plith-public.cer` exported from `CurrentUser\My` public part only)

**Interfaces:**
- `build.rs`: env `PLITH_SETUP_PAYLOAD` (folder with signed `plith.exe`, `plith-osd.exe`); copies
  them to `OUT_DIR`, or writes empty files when unset (dev, clippy, tests). winresource icon +
  VERSIONINFO (`OriginalFilename` `plith-setup.exe`); manifest through link args, not winresource.
- `payload.rs`: `pub struct Payload { plith: &'static [u8], helper: &'static [u8], cert:
  &'static [u8] }`, `pub fn embedded() -> Payload`, `pub fn is_release(&self) -> bool` (both
  exes non-empty).
- `processes.rs`: `pub trait Processes { fn close_plith(&mut self) -> Result<(), String>; fn
  stop_by_path(&mut self, exe: &Path) -> Result<(), String>; }`; `SystemProcesses` impl:
  `FindWindowW(w!("Plith.Notch"))` + `WM_CLOSE`, wait 5 s; Toolhelp snapshot +
  `QueryFullProcessImageNameW` exact path, `SeDebugPrivilege`, `TerminateProcess`, wait 5 s.
- `files.rs`: `pub fn install_files(root: &Path, payload: &Payload, setup_exe: &[u8], procs: &mut
  dyn Processes, log: &mut dyn FnMut(String)) -> Result<(), StepError>`; `pub struct StepError {
  pub reason: String }`. Order per spec section 5 step 1: write all `.new`, close, rename, remove
  not-ours (`plith_core::setup::is_ours`), ACL check (port `Get-AclProblems`: owner and every
  allow ACE with write bits must be SYSTEM, Administrators, TrustedInstaller or CREATOR OWNER;
  skipped when `root` is not under `Program Files`, so temp-dir tests run).
- [x] Step 1: tests on a temp root with a fake `Processes`: `fresh_root_gets_three_files`,
  `wpf_files_removed_after_rename` (seed `Plith.exe`, `Plith.dll`, `runtimes\x\y.dll`,
  `Setup\Plith-Uninstaller.exe`), `close_happens_after_new_files_are_written` (fake records
  order), `locked_target_fails_naming_file_and_leaves_no_new` (hold the target open with
  `share_mode(0)`), `rerun_after_failure_succeeds`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(setup): payload and file step`.

### Task 8: trust, register and uninstall steps

**Files:**
- Create: `crates/plith-setup/src/register.rs`, `register/tests.rs`, `src/uninstall.rs`,
  `uninstall/tests.rs`

**Interfaces:**
- Consumes: `plith_host::trust::*` (Task 6), `install_files` (Task 7).
- Produces: `pub fn trust_helper(scope: Scope, cert: &[u8], helper: &Path) -> Result<(),
  StepError>` (add to Root and TrustedPublisher if missing, then `verify_plith(helper)` must be
  Ok); `pub struct Places { apps_key: String /* HKLM path, or an HKCU test key */, start_menu:
  PathBuf }`; `pub fn register(places: &Places, root: &Path, version: Version) ->
  Result<(), StepError>` (link via `IShellLinkW` + `IPersistFile`, values from spec section 5:
  DisplayName `Plith`, DisplayVersion, Publisher `Plith`, DisplayIcon, InstallLocation,
  UninstallString `"<root>\setup\plith-setup.exe" --uninstall`, EstimatedSize KB, NoModify 1,
  NoRepair 1); `pub fn read_installed(places) -> Option<String>` (DisplayVersion);
  `pub fn uninstall(root, places, scope, procs, log) -> Result<(), StepError>`: close, delete
  files, `MoveFileExW(MOVEFILE_DELAY_UNTIL_REBOOT)` for the running `setup\plith-setup.exe` and
  the empty folders, remove link, key, certificate from both stores.
- [x] Step 1: tests with an HKCU test key, a temp start-menu folder and `CurrentUserTest`
  scope: `register_writes_every_value`, `register_replaces_wpf_values`,
  `read_installed_reads_display_version`, `uninstall_removes_link_key_and_cert`,
  `uninstall_on_nothing_installed_is_ok`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(setup): trust, register, uninstall steps`.

### Task 9: elevation, worker and pipe

**Files:**
- Create: `crates/plith-setup/src/elevate.rs`, `src/worker.rs`, `src/pipe.rs`, `src/pipe/tests.rs`
- Modify: `crates/plith-ipc/src/security.rs` (add `pub fn setup_pipe_sddl(user_sid: &str) ->
  String` = user and BA full, medium label) or keep it in `pipe.rs` if `security.rs` would cross 500

**Interfaces:**
- `pub fn start_worker(job: Job, pipe: &str) -> Result<WorkerProcess, ElevateError>`;
  `pub enum ElevateError { Cancelled, Os(String) }` (`ERROR_CANCELLED` 1223 maps to `Cancelled`);
  `ShellExecuteExW` verb `runas`, file = `current_exe()`, parameters built with `quote_arg`.
- `pipe.rs`: `pub struct ReportServer` (inbound, one instance, message mode, reject remote,
  SDDL above), `bind(name) -> Result<Self, String>`, `fn next(&self, timeout: Duration) ->
  Option<Report>`; `pub fn report_client(name) -> Result<ReportClient, String>` with `send(&Report)`;
  name `\\.\pipe\plith-setup-<session>-<window pid>`.
- `worker.rs`: `pub fn run(job: Job, pipe: &str) -> i32`: exit 2 without a write if
  `!own_integrity_above_medium()` or `verify_plith(current_exe)` fails or `!payload.is_release()`;
  else steps 1 to 3 (install) or uninstall, `Report::Step(n)` after each, `Failed` and exit 1 on
  error, `Finished` and exit 0.
- [x] Step 1: tests: `pipe_carries_reports_in_order` (server and client in one process),
  `worker_refuses_at_medium` (calls `run` in the test process: medium, so exit 2, temp marker
  untouched), `cancelled_maps_from_1223`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(setup): elevated worker and report pipe`.

### Task 10: setup window and app flow

**Files:**
- Create: `crates/plith-shell/src/setup_window.rs` (+ `setup_window/proc.rs`): borderless
  `WS_POPUP`, fixed 640 x 420 DIP, `WS_EX_NOREDIRECTIONBITMAP`, rounded corners (DWM), class
  `Plith.Setup`, icon resource 1, `WM_NCHITTEST` gives `HTCAPTION` where the screen says `Drag`,
  events `SetupEvent { Click(f64, f64), Key(u32), Dpi(f64), Closed }`, follows `WM_DPICHANGED`.
- Create: `crates/plith-core/src/setup/flow.rs` + `flow/tests.rs`: `pub struct Flow`,
  `pub enum Effect { Elevate(Job), StartPlith, SetAutostart(bool), DeleteUserData, Log(String),
  Quit }`, `pub fn Flow::new(mode, situation) -> Flow`, `fn on_action(&mut self, SetupAction) ->
  Vec<Effect>`, `fn on_report(&mut self, Report) -> Vec<Effect>`, `fn on_elevate_failed(&mut self,
  cancelled: bool)`, `fn screen(&self) -> Screen`.
- Create: `crates/plith-setup/src/app.rs`, `src/app/effects.rs`: mutex `Local\Plith.Setup`
  (second instance raises the first window and exits), language from
  `plith_host::locale::windows_language()`, art decode once (WIC, 1x or 2x by scale), `SheetRender`
  with `Palette::setup()`, pipe pump on a thread posting to the window, `setup.log` through
  `plith_host::log::LogFile` at `data_dir()\setup.log`. `StartPlith` = `ShellExecuteExW` open on
  `<root>\plith.exe` from this medium process. Done screen's autostart starts ticked when the Run
  value exists or is absent (spec), its toggle writes through `RegistryAutostart`.
- [x] Step 1: flow tests: `start_install_elevates`, `reports_press_ripples_then_done`,
  `cancelled_uac_returns_to_start_with_line`, `failure_shows_error_and_retry_elevates_again`,
  `newer_installed_offers_only_close`, `update_mode_shows_versions`,
  `uninstall_removes_run_value_and_deletes_data_when_ticked`, `done_open_starts_plith_and_quits`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: `cargo run -p plith-setup` (dev build, empty payload): window opens, start screen
  matches the Task 5 render (screenshot through `PrintWindow`), `KUR` reaches the UAC request;
  cancel it and see the cancelled line. Write what was seen in the progress log.
- [x] Step 6: commit `feat(setup): window and flow`.

### Task 11: release script

**Files:**
- Create: `scripts/build-setup.ps1` (pattern of `scripts/build-osd.ps1`: cert lookup never
  creates one, signtool lookup)

- [x] Step 1: script: release build of `plith` and `plith-osd` (`PLITH_OSD_UIACCESS=1`) through
  `heavy-run.mjs`, sign both, copy to a fresh `target\setup-payload-<stamp>\`, set
  `PLITH_SETUP_PAYLOAD`, release build `plith-setup`, sign, copy to
  `release\Plith-Setup-<version>.exe` (version from `cargo metadata`), verify all three with
  `Get-AuthenticodeSignature` (Valid, pinned thumbprint), print the three SHA-256.
- [x] Step 2: run it; expected: three `Valid` lines and the file in `release\`.
- [x] Step 3: commit `build: signed setup release script`.

### Task 12: real run on this PC (Berke's yes, his UAC click)

- [x] Step 1: ask Berke; record the state first: `Program Files\Plith` listing, Apps entry, Run
  value, cert stores.
- [x] Step 2: run `release\Plith-Setup-0.4.0.exe`: over WPF 0.3.3. Check: WPF files gone, three
  files present, Apps entry `0.4.0`, link present, Run value to the new path, `plith.log` shows
  the helper connected and `remote false`/integrity medium for `plith.exe`.
- [x] Step 3: run it again: repair path, same end state.
- [x] Step 4: uninstall from Settings > Apps: no files (except `setup\` until reboot), no entry,
  no link, no certificate in Root and TrustedPublisher, user data kept.
- [x] Step 5: reinstall; then rerun `scripts/install-osd.ps1` only if a dev helper is needed (not needed).
- [x] Step 6: record in this plan's log and in `quality/2026-11-xx-setup-first-run.md`; commit
  `docs: setup first run record`.

### Task 13: WPF settings import

**Files:**
- Create: `crates/plith-core/src/settings/wpf.rs`, `settings/wpf/tests.rs`
- Modify: `crates/plith-core/src/settings.rs` (357, `pub mod wpf;`),
  `crates/plith/src/settings_file.rs` (76: in `load_at`'s `NotFound` arm, read `config.ini`
  beside the path; if present, `wpf::import` then save and log `import: wpf config.ini, <n> keys`)

**Interfaces:**
- `pub fn import(config_ini: &str) -> (Settings, usize)`: from `Settings::default()`;
  `[General] Theme` Dark `gece`, Light `kagit`, Auto default; `[Osd] ShowWeather` True/False to
  `weather`; `[Osd] WeatherLocation` non-empty to `city` and `location = City`; count of keys taken.
  Section-aware INI read, BOM and CRLF tolerated, unknown keys ignored.
- [x] Step 1: tests: `berkes_config_maps_to_gece_weather_on_no_city` (the file read in
  brainstorming: Theme Dark, ShowWeather True, WeatherLocation empty; expect theme Gece, weather
  on, location default, 2 keys), `light_theme_maps_to_kagit`, `city_sets_city_mode`,
  `bom_and_crlf_tolerated`, `empty_text_gives_defaults`; `settings_file` test:
  `import_runs_only_without_settings_ini` (temp dir with both files: settings.ini wins).
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(settings): import wpf config.ini once`.

### Task 14: update rules and settings keys

**Files:**
- Create: `crates/plith-core/src/update.rs`, `update/tests.rs`
- Modify: `crates/plith-core/src/settings.rs` (keys 14 to 17: `update_check` bool default on,
  `update_checked` unix seconds `u64` default 0, `update_seen` version text default empty),
  `settings/tests.rs`, `settings/change.rs` (`SettingChange::UpdateCheck`)

**Interfaces:**
- `pub struct Release { pub version: Version, pub setup_url: String, pub page_url: String }`;
  `pub fn parse_release(json: &str) -> Option<Release>` (`tag_name`, `html_url`, asset whose
  `name` is exactly `Plith-Setup-<version>.exe`, its `browser_download_url`; uses
  `plith_core::json`).
- `pub fn next_check_in(now: u64, started: u64, last_ok: u64, last_failed: Option<u64>) -> u64`
  seconds: 120 after start, 86_400 after the last success, 21_600 after a failure.
- `pub fn should_notice(release: &Version, own: Version, seen: &str) -> bool`.
- `pub enum UpdateRow { Current, Checking, Ready(Version), Downloading(u8), BadSignature, Failed }`.
- [x] Step 1: tests: `parses_github_latest_with_setup_asset`, `release_without_asset_is_none`,
  `first_check_two_minutes_after_start`, `daily_after_success_six_hours_after_failure`,
  `notice_once_per_version`, `settings_round_trip_update_keys`, `old_settings_file_gets_update_defaults`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(core): update rules and keys`.

### Task 15: update service, download and launch

**Files:**
- Create: `crates/plith-host/src/http/download.rs` (`pub fn download(url, dest: &Path,
  progress: &mut dyn FnMut(u8)) -> Result<(), String>`: HTTPS only, follows redirects to
  `objects.githubusercontent.com`, 200 MB cap, `.part` then rename), `crates/plith/src/update.rs`,
  `update/service.rs`, `update/tests.rs`
- Modify: `crates/plith-host/src/http.rs` (`get_with_accept` adding
  `Accept: application/vnd.github+json`), `crates/plith/src/services/start.rs` (180, start the
  thread `plith-svc-update` like weather), `crates/plith/src/app.rs` (438, wiring at most 6
  lines; offset by moving one helper out if it would pass 500)

**Interfaces:**
- `UpdateService::start(own: Version, settings: UpdateSettings, hooks: Hooks) -> (Self,
  Receiver<UpdateEvent>)`; `enum UpdateEvent { Row(UpdateRow), Notice(Release), Checked(u64),
  Seen(Version) }`; commands `CheckNow`, `Install(Release)`, `Enable(bool)`, `Stop`.
- Install: download to `data_dir()\updates\Plith-Setup-<v>.exe`, `verify_plith`, on failure delete
  and `Row(BadSignature)`; on success `ShellExecuteExW` open with `--update --from <own>`.
- Notice only when `plith_shell::user_busy()` is `None`; else wait for the next check.
- [x] Step 1: tests with a fake fetcher trait: `check_reports_ready_and_notice_once`,
  `busy_user_defers_notice`, `bad_signature_deletes_file`, `disabled_check_never_fetches_but_check_now_does`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: commit `feat(app): daily update check`.

### Task 16: tray notice and settings rows

**Files:**
- Modify: `crates/plith-shell/src/tray/notify.rs` (69, `pub(super) fn balloon(hwnd, title, body)`:
  `NIM_MODIFY`, `NIF_INFO`, `NIIF_USER | NIIF_LARGE_ICON`), `tray/decode.rs` (52,
  `NIN_BALLOONUSERCLICK` 0x405 to `Click::Notice`), `tray.rs` (107, `TrayHandle::notice`,
  `ShellEvent::TrayNotice`), `crates/plith/src/app/state/tray.rs` (55),
  `crates/plith-core/src/sheet.rs` (173, `SheetFacts.update: UpdateRow`, `SheetAction::UpdateCheck`,
  `SheetAction::UpdateInstall`), `sheet/about.rs` (46, row `Güncelleme`), `sheet/general.rs` (59,
  row 3 toggle), `lang/sheet.rs` + eight files (`RowUpdate`, `SubUpdate*` per state, `WordCheck`
  `DENETLE`, `WordInstall` `KUR`, `RowUpdateCheck`, `SubUpdateCheck`; `SubData` adds GitHub),
  the five `SheetFacts` builders (section 6 of the map), `crates/plith/src/app/sheet_side.rs`
  (353, `Step::UpdateCheck`, `Step::UpdateInstall`), `app/state/sheet.rs` (382)
- Create: `crates/plith-core/src/sheet/update_tests.rs` if `sheet/tests.rs` (588) would pass 800
- Strings: notice title `Plith <version> hazır`, body `Kurmak için tıkla` (and seven languages).

- [x] Step 1: tests: `about_shows_update_row_per_state` (each `UpdateRow` gives its text and
  `DENETLE` or `KUR` target), `general_update_toggle_flips_setting`, `notice_click_decodes`
  (`decode::click_of(0x405)`), lang tests extended by `ALL`.
- [x] Step 2: FAIL; Step 3: implement; Step 4: PASS.
- [x] Step 5: render Hakkında and Genel with `render_sheet` into a fresh folder, look at 1x and 2x.
- [x] Step 6: commit `feat(app): update notice and settings rows`.

### Task 17: close the part

- [x] Step 1: five checks green; test count written from the run's own output.
- [x] Step 2: end-to-end update path once with Berke: a local test release is not available, so
  `DENETLE` against the real GitHub latest (0.3.3 is older: row `GÜNCEL`); record.
- [x] Step 3: STATUS rewritten (next job, risks: standard-user helper, root certificate, update
  never seen end to end until 0.4.1), ROADMAP row, journal entry, this plan's boxes and banner.
- [ ] Step 4: ask Berke to merge; commit `docs: r7 part 2 record`.

## Progress log
(written while the plan runs, never back-filled)

- 2026-11-26 Task 12 done: seven real runs (over WPF, repair, uninstall, reinstall, three
  repairs), two bugs found and fixed (`f66c80a` star marker in the pending list, `5625860` kill
  refused while exiting). Record: `docs/quality/2026-11-25-setup-first-run.md`. Uninstall was run
  with the Apps entry's own command, not clicked from Settings. Tasks 1 to 11 and 13 to 16 and the
  final fix wave were finished earlier; their record is the branch log and the SDD ledger.
- 2026-11-26 Task 17: the update check never worked (NUL in the Accept header, `0e12f40`); after
  the fix and run 8 the scheduled check reached GitHub and the row reads `GÜNCEL`. Checks on
  `0e12f40`: 1604 passed, 1 ignored. STATUS, ROADMAP, journal, spec note (R11) and boxes done;
  merge asked.
- Rulings made while running the plan, in order (cost if wrong): R1 no `SetupText::Version`
  (one string); R2 `UacCancelled` is the start layout plus a line (none); R3 public cert as
  `PLITH_CERT` in `plith-host` (a file move); R4 the app writes `update_checked`/`update_seen`
  (small); R5 integrity read from the token, not the log (none; in practice the OSD link proved
  it); R6 situation edge cases (one branch); R7 README title size over the comps (a restyle); R8
  raw OS text to `setup.log`, the screen shows step and number (one string); R9 update verified
  through a held handle (small); R10 installer art single-copy from `docs/` (the crate reads a
  docs path); R11 autostart tick kept off across updates (one branch); R12 release build in
  `target\setup-release` (a second cache); R13 Task 13 run while 12 waited (none); R14 screens
  approved by delegation (a later restyle); R15 C1 sweep and C2 medium relaunch (see N1, N2).
  Task 7: a global mutex squat ruled not applicable; Task 8: a malformed pending list is logged
  and left. Parked: N1, N2, N3 (STATUS risks); deferred minors per task in the review files.
