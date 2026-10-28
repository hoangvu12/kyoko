# 84 — Plugin settings: readable permission dialog, no scroll jump

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). The plugin permission dialog is hard to read (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_7.png`: low-contrast text, the dialog blends into the cards behind it). Opening a plugin's settings scrolled the page back to the top once (another plugin didn't).

**Blocked by:** none

**Status:** resolved

- [x] Permission dialog uses the dialog tokens (opaque surface, scrim behind, readable text at WCAG AA); capture checked by eye
- [x] Opening a plugin's settings or toggling one keeps the page's scroll position; find why it jumped (a full page rebuild?) and fix; in-process UI test

## Answer

Merged to main as `e1e0e6d` (worker `542c2b19`, lane A; rebased onto `015818f`, full verify re-run by the orchestrator: all green apart from the `theme_links` and `alerts::the_hook_hears_only_alerts_the_burst_limit_would_show` load flakes, both passing alone; kyoko-only capture `settings-plugin-permissions` checked by eye; `parity mockup` `settings-plugins` 7.2%, same as Midnight 19). Docs: `docs/ui.md`, `docs/parity.md`.

- Root cause of the jump: the plugin list and a plugin's settings view swap inside one scroll box, and zgx clamps the offset to the last laid-out height. A short settings view pulled it to 0, so going back landed at the top; a long one kept the old offset and opened partway down. Restoring on mount isn't enough (zgx still measures the old view), so the restore waits for the list's height.
- The permission prompt is Midnight's dialog: opaque `bg_4` panel with `border` and `radius_lg`, centred on a full-window `overlay` scrim, 20 semibold title with X, scrolling body, footer band with Cancel and Allow (`kit.button`). Scrim, X and Escape close; slide-in, none under reduced motion. Contrast `text_3`/`text_4` on `bg_4` about 9:1 and 5.3:1.
- New reusable pieces in `ui/settings/dialog.luau`: `dialog.popup { id, label, open, width?, content }`, `popup_header(close_id, title, on_close)`, `popup_body`, `popup_footer`. zgx gives modals a 32-point margin; set `max_w`/`max_h` to remove it. The frame publishes its page scroll as `dialog.offsets[prefix]` and the window size as `dialog.window`.
- Test: `ui::scripts::plugin_page::the_plugins_page_keeps_its_scroll_and_asks_on_a_scrim` (fails without the fix). Capture presses ClearURLs' switch (`capture::CLEAR_URLS_SWITCH`).
- Deviation: the scrim also dims the window controls (Midnight 07's Rust scrim leaves them bright, but a nested Luau dialog needs its own).
- For later: any page that swaps sub-views inside the page scroll hits the same clamping; restore after the new view has its height (watch a `ui.ref`'s `.height` in an effect). Luau tests: reopening a modal plays a 250 ms slide that `wait_until` doesn't advance (loop on `frames(1)`); `texts()` includes closed modals' text (use `visible_bounds`).
