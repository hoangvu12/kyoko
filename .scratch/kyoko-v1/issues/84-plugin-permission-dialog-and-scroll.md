# 84 — Plugin settings: readable permission dialog, no scroll jump

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). The plugin permission dialog is hard to read (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_7.png`: low-contrast text, the dialog blends into the cards behind it). Opening a plugin's settings scrolled the page back to the top once (another plugin didn't).

**Blocked by:** none

**Status:** claimed

- [ ] Permission dialog uses the dialog tokens (opaque surface, scrim behind, readable text at WCAG AA); capture checked by eye
- [ ] Opening a plugin's settings or toggling one keeps the page's scroll position; find why it jumped (a full page rebuild?) and fix; in-process UI test
