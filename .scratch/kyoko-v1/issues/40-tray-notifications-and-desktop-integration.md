# 40 — Tray, notifications and desktop integration

**What to build:** kyoko integrates with Windows:

- a tray icon
- OS notifications that follow Discord's notification settings
- global hotkeys
- autostart
- image clipboard
- single instance
- deep links

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** claimed

- [ ] The tray shows unread state, and offers show, hide and quit
- [ ] OS notifications fire for mentions and DMs, following per-server and per-channel settings, under kyoko's app user model id
- [ ] Global hotkeys and autostart work
- [ ] Images can be copied to the clipboard
- [ ] A second launch focuses the running window and passes any deep link to it
