# 12 — Command replies: thinking, ephemeral and failures

**What to build:** Show interaction replies as Discord does: the interaction header ("hoangvu12 used /roll"), the pending "Sending command..." row, "<App> is thinking..." for deferred replies, ephemeral messages with the mention gradient and "Only you can see this · Dismiss message", "The application did not respond" when no reply comes in time, and "This interaction failed" under components. Ephemeral messages live only in memory and go away when dismissed.

**Blocked by:** 05, 10

**Status:** claimed

**Mockups:** `docs/design/mockups/command-response-states.html`

**Spec:** `docs/design/midnight.md`, Command replies and components

- [ ] Each state matches the mockup
- [ ] The timeout follows the web client
- [ ] Dismiss removes an ephemeral message locally
- [ ] Session-seam tests cover deferred, ephemeral and failed replies with mockbot
