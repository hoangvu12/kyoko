# 07 — Restyle menus, popovers, dialogs and toasts

**What to build:** Context menus (`--bg-3`, 1-point border, about 220 wide, items inset 8, the neutral `--hover` wash on hover, measured), the message menu in Discord's order with kyoko's actions only (`bridge::message_menu` changes to that order), the status menu, popouts, tooltips, every dialog (scrim black at 70%, `--bg-4`, footers), toasts as overlays that never move the timeline, the connection-lost bar and the lightbox (black at 90%).

**Blocked by:** 01; v1 28, 30, 32, 35, 36, 38

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/message-menu.html`, `docs/design/mockups/message-menu-own.html`, `docs/design/mockups/status-menu.html`, `docs/design/mockups/delete-message-dialog.html`, `docs/design/mockups/dialog-*.html`, `docs/design/mockups/toast-rate-limited.html`, `docs/design/mockups/connection-lost.html`, `docs/design/mockups/lightbox.html`

**Spec:** `docs/design/midnight.md`, Other surfaces, Menus

- [ ] Each surface matches its mockup
- [ ] The message menu order follows the spec and its bridge test is updated
- [ ] Toasts never shift the timeline
