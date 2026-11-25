# plith-host

Windows helpers shared by `plith.exe` and the setup program: the log file under
`%LOCALAPPDATA%\Plith`, the Windows display language, the HKCU Run value for start with Windows,
and a blocking HTTPS GET over WinHTTP. Moved out of `plith` and `plith-weather` without a change
in behavior.
