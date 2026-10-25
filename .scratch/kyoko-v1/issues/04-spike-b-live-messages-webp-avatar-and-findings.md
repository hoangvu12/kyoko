# 04 — Spike B: live messages, WebP avatar and findings

**What to build:** Opening a channel in the spike window shows new messages arriving live. One WebP avatar is fetched and decoded in Rust and drawn by zgx. The spike's findings are written up so later tickets can adjust.

**Blocked by:** 03 (Spike A: zgx window with a live server/channel list)

**Status:** ready-for-agent

- [ ] Opening a channel shows its recent messages and adds new ones live, on both mockcord and real Discord
- [ ] One WebP avatar is fetched, decoded in Rust and shown as a host-supplied image
- [ ] The capability bits and the shape of IDENTIFY and the REST headers are compared against real traffic, and the result is recorded
- [ ] Findings are written down: build times, memory, zgx issues, bridge behaviour, and anything that changes later tickets
