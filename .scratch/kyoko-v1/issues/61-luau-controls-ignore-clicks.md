# 61 — Luau controls ignore clicks

**What to build:** Clicks reach Luau controls again, and a check catches it if they stop.

**Blocked by:** none

**Status:** claimed

Found during 33: on main `d432d58`, pointer presses delivered in-process with zgx `Ui::dispatch` do nothing on Luau controls (top bar buttons, user panel, member rows), while Rust modals still respond. The `status-menu` and `message-context-menu` parity captures, which drive the pointer, no longer reach their screens. It reproduces on a clean `origin/main` checkout. Suspected: the layers added to the root `overlay()` in `apps/desktop/src/ui.rs` by 30, 31, 32 and 33, or 45's capture-phase plugin input hook. 32 noted that a bare popover in that overlay swallowed every click until it was wrapped in a zero-size absolute box. Whether real mouse clicks are affected is not yet known.

- [ ] The commit that broke it is found (bisect between `ff7cb1a` and `d432d58`, or earlier if needed) and the cause explained
- [ ] Clicks on Luau controls work again, in-process and with real input, without breaking the Rust overlays (profiles, switcher, pickers, lightbox, thread popouts, plugin notice)
- [ ] The `status-menu`, `message-context-menu` and `message-hover` captures drive again
- [ ] A test fails if a full-window layer in the root overlay swallows presses meant for the Luau tree
