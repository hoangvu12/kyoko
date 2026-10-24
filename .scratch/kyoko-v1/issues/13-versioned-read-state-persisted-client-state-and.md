# 13 — Versioned read state, persisted client state and late DMs

**What to build:** Unread badges stay correct everywhere, later launches get a smaller initial payload, and DMs that Discord sends after the initial payload appear.

**Blocked by:** 12 (Bounded state caches)

**Status:** ready-for-agent

- [ ] Read-state and guild-settings versions are tracked and persisted across launches
- [ ] IDENTIFY's client_state is built from the persisted versions
- [ ] Private channels that arrive after READY are added
- [ ] Unread state stays correct for servers that aren't open
- [ ] Session tests cover each case
