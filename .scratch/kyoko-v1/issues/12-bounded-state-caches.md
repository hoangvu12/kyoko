# 12 — Bounded state caches

**What to build:** kyoko's memory stays bounded no matter how long it runs or how many channels are opened.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** claimed

- [ ] Messages per channel are capped. A limited set of warm channels keep message bodies, and colder channels keep metadata only
- [ ] Profile and member caches are capped
- [ ] Reading back through history widens a channel's window temporarily, then shrinks it
- [ ] Session tests assert the caps after opening many channels and long histories
