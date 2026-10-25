# 35 — Community flows

**What to build:** Adding or joining a server, invites, forwarding messages and onboarding match oxidcord. Onboarding has no artificial delay, which is the built-in version of NoOnboardingDelay.

**Blocked by:** 23 (Channel sidebar)

**Status:** claimed

- [ ] Community screens match oxidcord in the parity captures
- [ ] Joining by invite, creating invites and forwarding messages work
- [ ] Onboarding completes without artificial waits and respects the risky-action locks

## Comments

From the spike (04): joins and invite acceptance need the web client's `X-Context-Properties` header; add it to the fingerprint module when this ticket builds those requests.

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
