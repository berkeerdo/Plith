# plith-host

Windows helpers shared by `plith.exe` and the setup program: the log file under
`%LOCALAPPDATA%\Plith`, the Windows display language, the HKCU Run value for start with Windows,
and a blocking HTTPS GET over WinHTTP. Moved out of `plith` and `plith-weather` without a change
in behavior.

`trust` checks that a file carries a valid Authenticode signature by the Plith certificate (pinned
thumbprint; an untrusted root is forgiven only for that leaf) and adds, finds and removes that
certificate in the system stores. `assets/plith-public.cer` is the public certificate only.
