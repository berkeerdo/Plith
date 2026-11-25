# Setup first run on this PC (R7 part 2, Task 12)

Date: 2026-11-25 to 2026-11-26. Machine: Berke's PC, Windows 11 Pro build 26300, session over RDP
(`plith.log` start lines say `remote true`; nothing here depends on a console session).
Berke clicked every UAC prompt. State was read with a script after each run: the
`Program Files\Plith` listing, the HKLM Apps entry, the Start Menu link, the HKCU Run value, the
certificate `144A7919...` in LocalMachine Root and TrustedPublisher, Plith processes, and
`PendingFileRenameOperations` entries under the root.

## Runs

| # | Time | Build | Path | Result |
|---|---|---|---|---|
| 1 | 11-25 18:28 | `813FDFD5...` | over WPF 0.3.3 | Pass. WPF files and old uninstaller gone, 3 files, Apps 0.4.0, link and Run value to the new `plith.exe`, cert in both stores, `osd: link connected` (the helper trusts only a server above it, so `plith.exe` runs at medium) |
| 2 | 11-25 19:15 | same | repair | Pass. Plith closed and restarted, OSD connected again |
| 3 | 11-25 20:38 | same | uninstall (`--uninstall`, the Apps entry's command) | Pass. Only `setup\plith-setup.exe` left, it and both folders queued for restart delete; no Apps entry, link, Run value or cert; `settings.ini`, shelf and logs kept |
| 4 | 11-25 21:31 | same | reinstall | **Fail, found bug 1.** Install itself fine, but the three restart deletes from run 3 stayed queued: a reboot would have removed the setup copy and broken uninstall |
| 5 | 11-25 23:38 | `EED0AADB...` (fix 1) | repair | Restart deletes cancelled (log: `cancelling restart deletes left by an uninstall`). **Fail, found bug 2:** step 1 stopped with `cannot end process 24588: Access is denied` |
| 6 | 11-26 00:09 | `5BF91B06...` (fix 2) | repair | Pass. Plith was not running before this run, so bug 2's window did not occur |
| 7 | 11-26 00:10 | same | repair with Plith running | Pass. The sweep started at .354 while the OSD was exiting (`osd done` .373); files replaced at .384, Plith and OSD back, link connected. Its daily check at 00:12 still failed: **bug 3** |
| 8 | 11-26 00:21 | `40A12AC9...` (fix 3) | repair, then the first check | Pass. Plith started 00:21:45; the check 120 s later wrote `update_checked` 00:23:45 with no failure line and an empty `update_seen` (GitHub latest is 0.3.3, older): the About row reads `GÜNCEL` |

## Bugs found and fixed

1. **Star marker in the pending list** (`f66c80a`). On this build every
   `PendingFileRenameOperations` source starts with `*1` or `*2` before `\??\`
   (`*1\??\C:\Program Files\Plith\Setup`). The root match stripped only `\??\`, so nothing ever
   matched. Unit tests had used the documented shape without the marker. Fix: strip
   `*<digits>` only when the NT prefix follows; test with the measured shape.
2. **Kill refused for a process already exiting** (`5625860`). The OSD gets Plith's bye and exits
   on its own; `TerminateProcess` 10 ms later, while it was inside `ExitProcess` but not yet
   signaled, failed with access denied and the install stopped. Fix: on a refused kill, wait up
   to 5 s for the exit before failing. No unit test (a timing race in Win32); run 7 is the
   evidence, and the log cannot show whether the wait branch or a clean exit carried it. The Task
   7 review had flagged this exact line ("0 ms exit recheck after failed TerminateProcess") as a
   deferred minor; the real run turned it into a failed install.
3. **NUL in the Accept header** (`0e12f40`). `WinHttpSendRequest` in the `windows` crate takes
   the header length from the slice, and the header was built with a trailing NUL, so every
   update check failed with `The parameter is incorrect` (first seen at 11-25 21:33). Weather
   sends no header and kept working; the update tests use a fake backend. Fix: build the header
   without the NUL; unit test plus an ignored live test against GitHub, which failed with the old
   line and passes with the new one.

## Not measured here

- The elevated-start relaunch (C2) and the UAC prompt's owner window.
- The update path past the check: download, signature, the update screen and the restart
  (needs a published release newer than the installed one). The `DENETLE` button was not
  clicked; the scheduled check measured the same request.
- A standard (non-admin) user account.
- The list entry `[60..63]` on this PC pairs Defender log renames oddly (source and destination
  both non-empty); it is Windows' own and was left untouched, as designed.
