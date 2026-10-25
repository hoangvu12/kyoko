# 11 — Risky-action locks

**What to build:** Before actions that commonly trip Discord's spam defences, kyoko locks the action, explains why and asks for confirmation. The safety setting can turn this off.

**Blocked by:** 09 (REST safety layer)

**Status:** claimed

- [ ] Locks cover new DMs to strangers, message-request and spam DMs, empty channels, and server screening, onboarding and verification gates
- [ ] The locked state and its reason are exposed in state for the UI
- [ ] A single safety setting, on by default, turns the locks off after a restart
- [ ] Session tests cover each lock and the setting
