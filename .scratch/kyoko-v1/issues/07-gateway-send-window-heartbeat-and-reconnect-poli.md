# 07 — Gateway send window, heartbeat and reconnect policy

**What to build:** The gateway connection never floods Discord, notices when a connection has died, and reconnects correctly without looping on fatal errors.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** ready-for-agent

- [ ] Outbound frames, heartbeats included, never exceed 120 per 60 seconds. A priority lane keeps heartbeats and urgent frames ahead of bulk work
- [ ] The first heartbeat is jittered. A missing ACK closes the connection and resumes
- [ ] The choice between resume and re-identify follows the session's state, and INVALID_SESSION is handled
- [ ] Fatal close codes stop reconnecting and show a reason. Other drops back off, up to a cap
- [ ] Session tests cover each case using mockcord's scenario controls
