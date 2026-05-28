# Third-Party Notices

Plith incorporates code and techniques from the following open-source projects.

---

## VoicemeeterFancyOSD

- Repository: https://github.com/A-tG/VoicemeeterFancyOSD
- License: MIT
- Copyright: A-tG and contributors

The Host/Bridge/Interop layer enabling topmost-overlay behavior over fullscreen exclusive applications (BandWindow API approach with renamed `ApplicationFrameHost.exe`) is adapted from this project. UI/UX is written from scratch.

```
MIT License

Copyright (c) A-tG

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## ModernFlyouts (Community)

- Repository: https://github.com/ModernFlyouts-Community/ModernFlyouts
- License: MIT
- Copyright: ModernFlyouts Community

Techniques for hooking Windows volume events (used in Phase 3 fallback mode) are inspired by this project. No code copied directly; approach documented in their issue tracker and PRs.

---

## VoicemeeterRemote API

- Provider: VB-Audio Software (Vincent Burel)
- License: Voicemeeter EULA (free for end-user; redistribution of `VoicemeeterRemote64.dll` is allowed within Voicemeeter installations)

Plith uses the `VoicemeeterRemote64.dll` shipped with the user's Voicemeeter installation at `C:\Program Files (x86)\VB\Voicemeeter\`. Plith does **not** bundle or redistribute this DLL.
