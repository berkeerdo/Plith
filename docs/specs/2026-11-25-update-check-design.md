# Update check design

Date: 2026-11-25. Status: approved by Berke 2026-11-25 (approach 3, "önerin ne ise devam edelim
lütfen onaylıyorum"). Part of R7 parity part 2 (`research/2026-11-24-r7-parity.md`): it needs the
Rust installer, which is built in the same part.

## 1. Intent
Plith 0.4.0 has no way to learn about a new version. WPF had a manual "check" in Settings that
downloaded `Plith-Setup-*.exe` from GitHub and ran it. Berke asked whether updates need a page,
live in settings, or show a notice "from the bottom". A person who never opens Hakkında should
still learn that an update exists, and nothing should ever install behind their back.

## 2. Goals, non-goals, anti-goals
Goals
- A background check at most once a day, a Windows notice once per new version, and a Hakkında
  row with the state and a `KUR` button.
- `KUR` (or a click on the notice) downloads the setup, verifies it is signed by the Plith
  certificate, and opens the installer's update screen (A Halka, `0.4.0 → 0.5.0`, `GÜNCELLE`),
  which asks UAC once.
- A Genel row `Güncellemeleri denetle` turns the background check off; the Hakkında button still
  checks on demand.

Non-goals
- Silent or unattended install, delta updates, rollback, beta channels, release notes inside the
  app (the notice links to the GitHub release page instead).

Anti-goals
- Installing anything without a click. Running a download whose signer is not the pinned Plith
  certificate.
- A notice over a game: when `SHQueryUserNotificationState` says busy, fullscreen or presentation,
  the notice waits for the next check.
- Wakes on the UI or render thread for the check; the idle check stays at 0 there.

## 3. Constraints
- The OSD helper lives in `Program Files` and is trusted through the self-signed `CN=Plith
  Self-Signed` certificate, so an update always needs one UAC prompt (the installer is
  `requireAdministrator`). SmartScreen may warn on the downloaded file.
- WPF 0.3.x users arrive through their own GitHub check, so the release asset name stays
  `Plith-Setup-<version>.exe` (STATUS, R7 part 2).
- Pure logic (version compare, the once-a-day and once-per-version rules, the row state) lives in
  `plith-core` with unit tests; HTTP, WinVerifyTrust and the process start stay outside it.

## 4. Approaches (picked in conversation)
1. Settings only: a check and `KUR` in Hakkında. Simple, but nobody who skips Hakkında learns of
   updates.
2. Silent auto-update: not safe with a self-signed certificate and a UAC-gated helper.
3. **Daily check, one notice per version, install on click (picked).**

## 5. Decided
- Source: `GET https://api.github.com/repos/berkeerdo/Plith/releases/latest`; the newer version
  must carry an asset `Plith-Setup-<version>.exe`. Versions compare as semver against the exe's
  own version.
- Timing: first check 2 minutes after start, then every 24 hours from the last success; a failed
  check retries after 6 hours. The time of the last check and the last noticed version are kept
  in `settings.ini` (`update_checked`, `update_seen`).
- Notice: a tray balloon (`NIF_INFO`) titled `Plith <version> hazır`, body `Kurmak için tıkla`,
  shown once per version; a click on it starts the download. Windows shows it in the bottom
  right and keeps it in the notification centre.
- Hakkında row `Güncelleme`, states: `GÜNCEL` with `DENETLE`, `<version> HAZIR` with `KUR`,
  `İNDİRİLİYOR <n>%`, `İMZA GEÇERSİZ` (file deleted, nothing run).
- Download to `%LOCALAPPDATA%\Plith\updates\`, then WinVerifyTrust and a signer thumbprint equal
  to the pinned Plith certificate; only then start the installer with `--update --from <old>`.
- The installer closes the running Plith (`WM_CLOSE` to `Plith.Notch`), installs, and starts the
  new one at medium integrity.
- Genel row `Güncellemeleri denetle`, key `update_check`, default on (my call: an update nobody
  hears about helps nobody; the request carries no data but the IP, stated in the `Veri` row).
- Every new string in all eight languages (ADR 0015).

## 6. Left to the builder
- Which thread runs the check (the services pattern, a timer thread that sleeps between checks).
- The HTTP client (WinHTTP through `windows`, no new runtime dependency if avoidable).
- How the elevated installer starts Plith at medium integrity (for example through the shell's
  token).
