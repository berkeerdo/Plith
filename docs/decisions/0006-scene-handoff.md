# 0006 Scene handoff between threads

Date: 2026-11-14. Status: accepted. Drafted by lane `engine` from the R1 console session
(release build, RTX 5080, 240 Hz, `%LOCALAPPDATA%\Plith\plith.log`); numbers copied from the log.

**Decision:** a latest-wins mailbox (`Mutex` + `Condvar` + generation): the UI and demo threads
`update` it atomically, the render thread parks in `wait_newer` at rest and reads `latest()`
each frame while moving. Not a channel: intermediate targets are meaningless.

**Measured:** `--idle-check 60`: 0 render wake-ups in 60 s, 62.5 ms process CPU (G2). Display
off, lock and fullscreen each parked the render thread mid-motion with no motion lines until
return (display off 3.6 s at 21:24:02; lock 18.5 s at 21:29:11; fullscreen during Deadlock).
