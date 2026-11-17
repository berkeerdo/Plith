# Oyenza Studio UI research (clone: oyenza-src-2, depth 1)

Paths are relative to oyenza-src-2/crates/poni/src unless stated. Not built or run.

## 1. Settings

Two surfaces: (a) the island's Settings page (gear, top right of the open island), (b) the "Advanced settings" window (settings_window.rs, 2283 lines; card 840x580 DIP, CARD_W/CARD_H at lines 29-30), opened from the last row of (a).

### 1a. Island Settings page (pages/settings.rs), top to bottom
- Account row (Oyenza icon, sign in / who is signed in / Disconnect), then a rule (RULE_Y 54).
- Pickers under captions "THEME" and "IDLE LOOK" (settings.rs:186-187). If narrow, one row each. Theme choices: Classic, Oyen, Glass, Pill, Diorama (settings.rs:89). Idle look choices come from status::looks(theme) (pill/status.rs:62-70): Classic/Oyen/Glass = Clock, Eyes (named "Oyen" in the Oyen theme), Minimal, Hidden; Pill adds Status; Diorama only Clock, Minimal, Hidden (settings.rs:92-107).
- Two columns of rows (settings.rs:128-136): left "Synced lyrics", "Lyrics pronunciation", "Sounds"; right "Version <x>" with "Check now" (becomes "Checking...", "Up to date", "Try again", or an "Update to <ver>" pill), "In full-screen apps" with choice Hide / Stay on top (settings.rs:142), "Start with Windows" ("Start at login" on Mac, l.226).
- "Tabs" row of chips (settings.rs:295-346): one chip per hideable page (icon + name; off = icon struck through).
- Last row, under a rule: "Advanced settings" with an open icon (settings.rs:284).

### 1b. Advanced settings window: 13 sidebar sections in order (settings_window.rs:165-195), content built in blocks() (l.~457-650). Each row = label, dim sub line, control (switch / choice / stepper / button). Window heading text "Advanced settings" (l.1252). Follows system light/dark (l.16-17).
1. General (l.464): Version block (Check now / update buttons, l.1585-1627); "Start with Windows" ("Opens Oyenza Studio when you sign in", l.398; Mac: "Start at login" / "Opens Oyenza Studio when you log in"); "Sounds" ("A soft sound for timers, drops and reminders"); "24-hour clock"; "Keep System page history" ("The charts keep the last minute while the page is closed", only if the System add-on is on).
2. Appearance (l.474): "Theme" cards (heading l.1653), five cards with captions (l.388-392): Classic "A black island at the top edge"; Oyen "Oyen lives in the island"; Glass "Frosted glass over your screen"; Pill "Floats just below the top edge"; Diorama "A little place that keeps your day". Then group "Island at rest": "Idle look" / "What the closed island shows" (choice).
3. Island (l.487): group "Opening and closing": "Opens after" (Quick 120 ms / Normal 220 / Slow 450; "How long the pointer rests on the island first"), "Closes" (At once / Short 400 ms / Long 1000 ms; "Once the pointer leaves the open island"), "Width when open" (stepper; "Wider gives the pages more room"). Group "Where it shows": "Display" (Main display / Under the pointer; Windows only; "The screen the island hangs on"), "In full-screen apps" (Hide / Stay on top; "Games and videos that fill the screen"), "Show FPS, GPU and CPU over games" ("Keeps the island on top of full-screen apps"), "Only for a few seconds" ("Then the island hides and adds no delay to the game").
4. Music and lyrics (l.509): group "Lyrics": "Synced lyrics" ("Lyrics move with the song, from LRCLIB"), "Lyrics pronunciation" ("Romaji, pinyin and Korean under the lines"), "Timing" stepper ("Shift every line earlier or later", shows "On time" or a signed seconds value, l.454). Group "Closed island": "Show what's playing" ("The song and its cover on the closed island").
5. Pop-ups (l.520-527): one group titled "The island shows a pop-up for": "Volume" ("When you change the volume or mute", hud_volume), "Charger and battery" ("When you plug in, unplug or run low", hud_power), "Bluetooth" ("When headphones and other devices connect", hud_bluetooth). All default true (poni-core/src/settings.rs:279-281). Sidebar icon is Icon::Volume (l.224).
6. Tabs (l.528): group "When the island opens": "Open where you left off" ("The page you last had open"), and when that is off "Opens on" stepper; group "Tabs in the island": one switch per page (Notifications has sub "Messages from WhatsApp, Telegram and your phone"); note "Oyenza and the add-ons' pages always have a tab."
7. Add-ons (l.546): add-ons list block.
8. Focus (l.548): "Lengths": Focus, Short break, Long break (minutes steppers), "Long break after" ("Focus sessions before a long break"); "Starting by itself": "Breaks" ("A break starts when a focus session ends"), "Focus sessions" ("The next session starts when a break ends").
9. Prayer times (l.572): "Times and Hijri dates": "Follow" (Kemenag / Muhammadiyah (KHGT)); "When it's time": "Sound" (Adzan/Tone/Silent), "Pause music for the adzan", "Countdown before each prayer" ("On the closed island"); "Place": "Follow my location"; note "Pick a place by hand on the island's Prayer page."
10. Caffeine (l.605): "Last call": Bedtime, Half-life, "Left at bedtime", "Remind me before last call"; "Day limit"; note.
11. AI usage (l.598): "Heads-up notices", "Close to the limit at" (shows "N %"); note.
12. Oyenza account (l.622): account block.
13. About (l.623-640): Version block, "What's new" (Open), "Lyrics" (lrclib.net), group "Files": "Settings file" (settings.json), "Data folder".

### Idle options (what each draws)
Enum IdleStyle in poni-core/src/settings.rs:10-22. status::looks/shown (pill/status.rs:62-82): Status draws the clock outside Pill; Eyes draws Minimal in Diorama.
- clock (Classic/Oyen/Glass, activities.rs:268-277, draw 432-465): pill 84 DIP wide (24h) or 104 (12h) x 30; time semibold, centered; badge: green bolt when charging, red "NN%" when low, centered together with the time. Oyen theme adds Oyen peeking at left. In Pill theme (pill/idle.rs:101-130): 32 DIP capsule; time left, "NN%" + drawn battery right (152 wide 24h / 180 12h; 84/104 with no battery, time alone).
- minimal: blank capsule, nothing drawn (activities.rs:279 150x26; idle.rs:60 96x24), only something to hover.
- eyes: two blinking eyes that glance toward the cursor (activities.rs:471-475, 118x30; Oyen theme draws the Oyen sprite 104x36, l.468). Pill: 84x32 capsule.
- hidden: nothing visible, an invisible edge strip that catches the pointer (activities.rs:283 hidden_strip; idle.rs:62-63).
- status (Pill theme only, pill/status.rs:1-12, draw ~120-190): 32 DIP capsule: volume speaker glyph at left (VolumeMute/VolumeLow/Volume by level, same thresholds as the HUD, only if volume known), inner lighter 22 DIP pill with short date and time, right: "NN%" (red when low) and drawn battery with bolt when charging. No Bluetooth glyph, no chevrons or bell.

### HUD options (app.rs:2566-2600)
- hud_volume: on AppEvent::Volume, if not the first reading and hud_volume, show_hud(Volume{level,muted}, 1.6 s) (app.rs:2569-2570).
- hud_power: on AC change shows "Charging" / "On battery" for 2.2 s; "Low battery" pop-up when crossing 20% and 10%, 4.0 s.
- hud_bluetooth: show_hud(Bluetooth{name,connected,battery,audio}, 2.6 s) (app.rs:2585-2586), only for headphones/earbuds/speakers (services/bluetooth.rs:14).

## 2. Volume HUD

No device name anywhere in the volume HUD: HudKind::Volume carries only level and muted (app.rs:2570). The audio service watches the default render endpoint's master volume (services/audio.rs:1-12). No brightness HUD exists: "brightness" appears only in a comment (activities.rs:213) and backdrop code (SDR content brightness for glass); nothing shows on a brightness change.

Classic/Oyen/Glass (activities.rs:846-885, size l.313): capsule 316x40, radius 19.
- Icon at x+16, 20x20: VolumeMute if muted or level<=0.001, VolumeLow if <0.5, else Volume (icons.rs:276-278: speaker with 0 waves + X / 1 wave / 2 waves).
- Bar: starts x+48, height 6, width body.w-48-62 (stops 62 DIP short of the right), vertically centered; track palette::TRACK, radius 3; fill white 0.95 (theme accent in Oyen), min width 6.
- Bar glides: each frame cur += (target-cur)*0.32 until within 0.002 (l.865-872); muted target = 0 (bar empties). The number is exact immediately.
- Value: right-aligned in a 48 DIP box, 16 from the right end, semibold tabular: "NN%" with percent sign (round(level*100)), or "Muted" when muted (l.882-883).

Pill theme (pill/hud.rs:97-134): capsule 272x32; hand-drawn speaker at x+12 in a 16 DIP box (0, 1 or 2 arcs, X when muted); bar starts x+40, 5 thick, track white 0.22, fill white; value in dim color, medium tabular, right at 14 from the end, slot sized for "100%" or "Muted"; same 0.32 glide.

Duration 1.6 s (app.rs:2570); cleared when now >= until (app.rs:3560-3561). The pill morphs in and out; a notice that "stays" waits under a HUD (activities.rs:213-218). No sound, no device switching.

Bluetooth HUD for completeness: Classic 336x46 (activities.rs:315): 30 DIP disc (blue when connected, grey when not) with Headphones icon (audio) or Bluetooth icon; name semibold on line 1, ellipsized to the room left of the battery badge (text rect = right edge minus badge minus 12, activities.rs:898-906); line 2 footnote dim "Connected" / "Disconnected"; right: "NN%" + drawn battery (red <=20, else green) if reported. Name = Windows device Name, trimmed, fallback "Bluetooth device" (services/bluetooth.rs:187, 666, FALLBACK_NAME l.76); Mac fallback "Bluetooth headphones". Pill: single line "Name  Connected", width clamped 160..360 (pill/hud.rs:23-25, 239-244), name ellipsized (text_upto l.216), state never cut. Duration 2.6 s.

## 3. Audio/output/device UI in the expanded island
None found. Grep for mixer, output device, per-app volume, audio device, speaker across crates/poni/src (excluding mac/) returns only icon paths, the volume watcher, the notes recorder's own Mixer (call/notes recording, services/notes.rs) and log lines. The Music page (pages/media.rs; H_SESSION 148, draw_session l.333) holds the media session (cover, title, transport, lyrics column) with no device picker, no master slider, no per-app volume. Audio device handling is only: follow the default output endpoint (services/audio.rs) and the Bluetooth connect pop-up. The only explanatory text about audio is the Pop-ups settings sublines.

## 4. Images (mostly webp; the repo has few PNGs)
Clone root: C:\Users\listl\AppData\Local\Temp\claude\0a0194b114ae9939\bcaab6cf-f235-4905-9414-f01d4cfd10fc\scratchpad\oyenza-src-2\
1. site\assets\shots\classic\settings-window-general.webp (window, 840x600)
2. site\assets\shots\classic\settings-window-appearance.webp
3. site\assets\shots\classic\settings-window-island-on-top.webp (Island section)
4. site\assets\shots\classic\settings-window-tabs.webp
5. site\assets\shots\classic\page-settings.webp (island Settings page, 660x333)
6. docs\screenshots\06-volume-hud.png (PNG, volume HUD)
7. site\assets\shots\zoom\classic\volume-hud.webp (2x volume HUD)
8. site\assets\shots\pill\volume-hud.webp (Pill)
Also: site\assets\shots\pill\idle-status.webp (Status idle), docs\screenshots\08-bluetooth-hud.png, docs\brand\readme\glance.webp (six resting islands incl. volume). No audio-UI screenshot exists because no such UI exists.
