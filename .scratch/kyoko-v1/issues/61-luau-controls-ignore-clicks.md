# 61 — Luau controls ignore clicks

**What to build:** Clicks reach Luau controls again, and a check catches it if they stop.

**Blocked by:** none

**Status:** resolved

Found during 33: on main `d432d58`, pointer presses delivered in-process with zgx `Ui::dispatch` do nothing on Luau controls (top bar buttons, user panel, member rows), while Rust modals still respond. The `status-menu` and `message-context-menu` parity captures, which drive the pointer, no longer reach their screens. It reproduces on a clean `origin/main` checkout. Suspected: the layers added to the root `overlay()` in `apps/desktop/src/ui.rs` by 30, 31, 32 and 33, or 45's capture-phase plugin input hook. 32 noted that a bare popover in that overlay swallowed every click until it was wrapped in a zero-size absolute box. Whether real mouse clicks are affected is not yet known.

- [x] The commit that broke it is found (bisect between `ff7cb1a` and `d432d58`, or earlier if needed) and the cause explained
- [x] Clicks on Luau controls work again, in-process and with real input, without breaking the Rust overlays (profiles, switcher, pickers, lightbox, thread popouts, plugin notice)
- [x] The `status-menu`, `message-context-menu` and `message-hover` captures drive again
- [x] A test fails if a full-window layer in the root overlay swallows presses meant for the Luau tree

## Answer

Merged to main as `cf941a3`. Docs: "Root overlay layers" in `docs/ui.md`.

- Cause: `92a08c8` (31, quick switcher). `SwitcherView::view` returned a plain `div()` around its modal; as a non-absolute child of the root `overlay()` it stretched over the whole window even while closed. zgx sends a press to the topmost box under the pointer (containers count), then to that box's nearest ancestor with listeners, which was the root overlay (Escape, Ctrl+K, file drop, 45's plugin input hook). So every press below the switcher stopped at the root, silencing the whole Luau tree and the timeline slot (the right-click message menu too). Rust modals kept working because zgx draws them as portals. Real mouse clicks were affected as well. 45, 32 and 33 were not at fault. Bisected with a headless probe (passes with the profiles layer at `2a15eac`, fails with the switcher at `ff7cb1a`) and the pointer-driven `status-menu` capture (works at `e1b91fb`, not at `92a08c8`).
- Fix: the root is now `layered(host.view(), [...])` in `apps/desktop/src/ui.rs`, which puts every layer in a zero-size absolute box. Layers that need the window while open size themselves; zgx `modal`, `popover` and `menu` are portals.
- Test: `ui::root::presses_reach_the_luau_tree_through_every_closed_layer` (`apps/desktop/src/ui/root.rs`) builds the real Luau shell inside `layered(...)` with listeners on the root, adds each closed layer alone and then all together, clicks the top bar's threads and pins buttons and checks their actions run. On the broken composition it fails with `quick switcher swallowed ["top_bar_threads", "top_bar_pins"]`. `cargo xtask test` now runs `ui::root` as well as `ui::scripts`.
- Verified: an OS click sent only to the worker's own kyoko window opened the status menu; pointer-driven captures (port 8493) status-menu 6.3%, message-context-menu 6.5%, message-hover 6.1%; every Rust overlay (profiles, switcher, pickers, lightbox, thread popouts and dialogs, 35's community dialogs) captured and checked by eye. Checks re-run by the orchestrator in lane C.
- Rule for every later UI ticket: add a layer by appending its view to the `layered(host.view(), [...])` list in `ui.rs` and to `LAYERS` and the `layer` match in `src/ui/root.rs` (so the test covers it); never `.child(...)` it onto the overlay. A layer's root takes the window size from a window signal only while open, 0 by 0 otherwise. Never put listeners on a node that covers the window while the layer is closed.
