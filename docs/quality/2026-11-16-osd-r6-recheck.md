# Re-check: feature/osd-r6 fix round (75050ec..d6be46c)

Read-only. `git diff 75050ec..d6be46c` and `git show d6be46c:<path>`. Nothing built or run.
Line numbers are at d6be46c.

## Verdict

**Do not merge yet: one new Important defect (N1), and it reopens M4.** The fix is small, a
guard inside the scope. Everything else is closed, or partly closed with a residual that is
documented and low risk.

## Per finding

| Item | Result | Evidence |
|---|---|---|
| I1 install pins | **Closed for the scenario as written** (swap after build-osd). Residual is documented. | `install-osd.ps1:26` pins the thumbprint. `:146-152` requires `-Sha256`. `:159-161` checks hash and signer on the copy inside Program Files, before the rename. `:155,168` assert an admin-only ACL before and after. `:170-173` removes the exe if the ACL check fails (fail closed). `build-osd.ps1` prints the SHA-256. |
| I2 Hello pid | **Closed** | The kernel pid comes from `GetNamedPipeClientProcessId` (`server.rs:310-350`). `admission.rs` rejects a pid mismatch. `app.rs:228-233` cancels and joins the old watcher before it starts one for `Connected { pid }`. `watch.rs` waits on the process and a cancel event. The pid in Hello cannot steer `OpenProcess` any more. |
| I4 shared journal | **Closed** (one session) | `hook.rs:118` takes `lock::acquire` before the hook thread reads or clears the journal. `lock.rs:40-51` uses a named mutex and fails with `ERROR_ALREADY_EXISTS`. The field is dropped last. verify_writes now reports Fail. |
| M1 journal before hide | **Closed** | `registry.rs` (`RestoreHandle::hide`): insert, then mirror, then `post_hide`. A refused post removes the entry and mirrors again. Tests cover both. |
| M2 medium squatter | **Closed** | `identity.rs:64`: `Installed` requires a strictly higher integrity than the client. It is the default of `OsdLink::start`. AccessDenied fails closed (`client.rs` `connect`). |
| M3 pipe name | **Partly** | `pipe_name.rs` / `launch.rs:38`: the name is now `plith-osd-<session>-<sid>`. That stops accidental collisions. A SID is not secret, though (ProfileList and LSA lookups give it away), so another user who sets out to squat can still pre-create the name. Same Squatted DoS as before. A real fix needs a name the other user cannot predict, such as a per-launch random suffix passed as `--pipe`. Low severity. |
| M4 server panic | **Not closed (see N1)** | `QuitOnPanic` (`app.rs:284-298`) is correct, but its test is synthetic. A panic inside `OsdServer::serve` never unwinds to the guard. |
| M5 High log | **Closed, with a small residual** | `log.rs`: `SHGetKnownFolderPath`, reparse checks on the folder and the file, a final-path check, `{:?}` and control escaping (`rules.rs`), a 1 MiB cap per run and rotation at open. Residual: `open(create)` at `log.rs:97` runs before the final-path check at `:99`, so a junction swapped in during the race still makes the High process create an empty `plith-osd.log` in the target folder. It only appends and never truncates. Low. `FILE_FLAG_OPEN_REPARSE_POINT` or a process redirection-trust mitigation would close it. |
| M6 silent client | **Partly** | `HELLO_TIMEOUT` of 1 s (`server.rs:318`, `hello_clock.rs`) drops a client that is silent before Hello. The scenario in the report was "sends Hello, then stays silent". A same-user client that sends a correct Hello with its own pid still holds the single instance forever. Not checked against the launcher pid. Low, same-user DoS only. |

## New defects

### N1 (Important): a panic in `serve` hangs the helper; the M4 guard never runs
`plith-ipc/src/server.rs:232-240`.

`serve` runs `serve_clients` inside `std::thread::scope` next to the `plith-ipc-hello-clock`
thread. `shared.hello.quit()` runs only on the normal return path (`:238`). If `serve_clients`
panics (in `pump`, in `admit_client`, or in the `on` callback, which is `Served::on` in the
helper: logging, `watch::client`, the `Watcher` drop and join), here is what happens:
1. `std::thread::scope` catches the panic and waits for every scoped thread before it re-raises.
2. The clock thread sits in `run` with `deadline == None`. That is `Condvar::wait` with no
   timeout, and `quit` is never set.
3. The scope never returns. The server thread is stuck, and `QuitOnPanic` in
   `plith-osd/src/app.rs:298` is never dropped.
4. The render thread is never told to quit, and the pipe handle stays open with no reader.

That is the M4 zombie again: a High process with no working OSD. The notch sees
`has_exited() == false`. The new test (`app/tests.rs`,
`a_panic_on_the_server_thread_asks_the_renderer_to_quit`) drives `QuitOnPanic` on a bare thread,
not through `OsdServer::serve`, so it cannot see this.

Fix: in the scope closure, put a drop guard that calls `shared.hello.quit()`, for example
`struct QuitClock<'a>(&'a HelloClock); impl Drop { self.0.quit() }`, created before
`serve_clients`. Add a test where the `on` callback panics in a real `serve`, and assert that
`serve` unwinds and the guard sends `Quit`.

### N2 (Minor): `Installed` trust can lock out the real helper in two unmeasured cases
`identity.rs:64` requires the server level to be strictly above the client's level. Both cases
below fail closed: no data leaks, the OSD is simply missing.
- **The notch runs elevated** (run as admin, or UAC off). The helper is then High and so is the
  notch: they are equal, so the helper is refused. CLAUDE.md says plith.exe runs at medium, so
  this happens only by accident. Still, the wiring should detect it and log it.
- **A standard (non-admin) user.** The code comment admits that the UIAccess level being one
  step above medium is not measured. Measure it on a standard account before wiring.

### N3 (Minor): the I4 lock is per session, the journal is per user
`journal.rs:35` uses `Local\`. If the same user runs two sessions (console plus RDP, where
allowed), both see the same `%LOCALAPPDATA%` journal but get separate mutexes, so the I4 race
returns across sessions. `Global\` closes it; a mutex needs no privilege there. Low. The
mutex also uses the default DACL, so a same-session process can pre-create it to block hide
mode. That fails closed.

### N4 (Nit): install UX can defeat the hash pin
The dry run prints the source's current SHA-256 (`install-osd.ps1:132`). A developer who copies
that value, instead of the one build-osd printed, pins whatever is in `target\release` right
now. Print only "compare with build-osd", or print the hash with a warning next to it.
Separately, the I1 residual stays (accepted, ROADMAP): a swap between `cargo build` and
`signtool` inside build-osd, and the key reachable at medium. The script comment documents
both.

### N5 (Nit): odd spacing in a user-visible error string
`client.rs:27`, `UNTRUSTED`: "or an      integrity level" has a run of spaces, probably a
leftover from a line break.

### Checked and sound
- **Hello clock locking.** The clock lock is taken before the thread-handle lock in
  `cancel_serving_io`. Nothing takes them in the reverse order, because `Stopper::stop` and
  `Registration` take only the thread lock. Disarm under the clock lock prevents a late cancel.
  An expired clock re-cancels every 10 ms until it is disarmed. Stop is checked before expiry.
- **Watcher join.** The watcher's `events.send` can block only while the render thread (the
  consumer) is alive, and the render thread never waits on the server thread. Once the render
  thread ends, the receiver drops and the send fails at once. `Disconnected` still arms the 2 s
  exit in the model, so dropping the watcher on disconnect does not create a lingering helper.
- **Install script.** It removes only literal paths (`$dest`, `$temp`). The ACL mask covers
  write, append, attributes, delete-child, delete, WRITE_DAC, WRITE_OWNER and GENERIC_ALL/WRITE.
  The default inherited Program Files ACL (Users RX, app packages RX, CREATOR OWNER skipped)
  passes. A pre-existing user-writable `C:\Program Files\Plith` fails closed before the copy.
- **Journal.** journal-first plus restore-only-if-not-visible means a kill before the post
  cannot show a window that was never hidden.
- **No em dash** in the diff.

## febb38f (case rules)
**Correct.** `paint.rs:122` `upper_name` uses `str::to_uppercase`, which is locale-independent
Unicode mapping: `i` becomes `I` and `ı` becomes `I`, so `Display` reads `DISPLAY` and
`Mikrofon` reads `MIKROFON`. Window, device and app labels go through `upper_name`
(`paint.rs:239`). Plith's own word `sessiz` goes through `upper_tr` and becomes `SESSİZ`
(`paint.rs:51`). The tests cover both.

One thing to carry into wiring: the helper cannot tell a Windows name from a Plith label. If
the notch ever sends its own Turkish word as a label (for example a fallback "Parlaklık"), it
must upper-case it with `upper_tr` before sending, or it will render as "PARLAKLIK" with the
wrong I. No such literal exists on the branch today.
