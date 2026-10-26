# 07 — Restyle menus, popovers, dialogs and toasts

**What to build:** Context menus (`--bg-3`, 1-point border, about 220 wide, items inset 8, the neutral `--hover` wash on hover, measured), the message menu in Discord's order with kyoko's actions only (`bridge::message_menu` changes to that order), the status menu, popouts, tooltips, every dialog (scrim black at 70%, `--bg-4`, footers), toasts as overlays that never move the timeline, the connection-lost bar and the lightbox (black at 90%).

**Blocked by:** 01; v1 28, 30, 32, 35, 36, 38

**Status:** resolved

**Mockups:** `docs/design/mockups/message-menu.html`, `docs/design/mockups/message-menu-own.html`, `docs/design/mockups/status-menu.html`, `docs/design/mockups/delete-message-dialog.html`, `docs/design/mockups/dialog-*.html`, `docs/design/mockups/toast-rate-limited.html`, `docs/design/mockups/connection-lost.html`, `docs/design/mockups/lightbox.html`

**Spec:** `docs/design/midnight.md`, Other surfaces, Menus

- [x] Each surface matches its mockup
- [x] The message menu order follows the spec and its bridge test is updated
- [x] Toasts never shift the timeline

## Answer

Merged to main as `2d3499e`. Docs: `docs/ui.md`, `docs/parity.md`.

- Shared parts `apps/desktop/src/kit.rs`: menus (220 wide, `bg_3`, 1-point `border`, `radius_sm`, 32-point items inset 8, neutral `hover` wash, danger items `red_2`), dialogs (centred, 440 wide, `bg_4`, `radius_lg`, 20 semibold title with close X, footer band with a line above), primary/critical/secondary/link buttons, inputs and textareas (`bg_3`, `accent_2` focus), switches, radio rows, checkboxes per `MOCKUP_ROLES`. `community/parts.rs` and `moderation/kit.rs` delegate to it, so every timeline, community, moderation and thread dialog shares one look; the member menu, picker cell menus and the Luau server, channel, friends and forum menus use the same look.
- Message menu (`bridge::message_menu`) in Discord's order with kyoko's actions only: Add Reaction; Edit, Reply, Forward, Create Thread; Copy Text, Pin, Mark Unread, Copy Message Link; Select, Delete N Selected, Delete, Report; Copy Message ID (developer mode) with an ID badge; a row of up to four quick reactions (frequently used emoji) on top. Plugin items still last behind a separator with their ids. Test `the_message_menu_follows_ownership_and_permissions` asserts both orders.
- Scrim: new root layer `src/scrim.rs`, black 70% while any dialog is open, leaving out the window controls' box (they stay bright above the dim but take no clicks while a dialog is open: zgx's backdrop sits above them). zgx's own backdrop is transparent and still closes on an outside press. `motion::modal` registers the scrim; a dialog with its own scrim calls `scrim::track(&open)`.
- Dialogs: Delete Message and Pin show the message preview (Delete adds the protip); Kick, Ban, Timeout use Discord's wording and a reason textarea (Timeout now sends its reason); Change Nickname has Reset Nickname; Notification Settings uses radio rows and Done; Forward has 44-point rows with checkboxes and Send (n); Add a Server opens on Discord's start screen (Create My Own, Join a Server); custom status (now centred) and "Leave Kyoko?" restyled.
- Toasts: 360 wide, centred over the timeline 8 above the slot's bottom (they follow the composer's height), rising 24 points in. Test `ui::root::a_toast_floats_over_the_timeline_and_never_moves_its_rows`. Connection bar restyled in its Luau slot as Midnight's card (dots, status, Retry). Lightbox black 90% with a toolbar (file name, Open original, Close). Status menu: same panel and rows with a check badge.
- Checks re-run by the orchestrator in lane A: all green. Mockup parity (port 8494): message-context-menu 17.7%, message-context-menu-own 17.9% (new), delete-message-dialog 14.2% (new), status-menu 18.6%, custom-status 16.3%, lightbox 50.9% (test image vs striped placeholder), add-server 24.1%, forward 18.0%, guild-notifications 17.4% (centred and dimmed now, content behind differs), moderation-nickname 13.4%, -timeout 12.9%, -kick 11.8%, -ban 12.4%, toast 17.6%, connection-offline 15.8%. By eye positions, sizes and colours line up.
- Left out (no data or action): Add a Server templates, Forward's optional message, Notification Settings' Mute / Suppress Highlights / Mute New Events, the lightbox's file size and author, an Add Reaction submenu. Status menu's account-popout layout and the custom status selects are M20's; thread dialog content is M22's. "Reset Nickname" in `text_3`; Cancel/link buttons can't take keyboard focus.
- For later tickets: build dialogs with `kit::dialog(motion::modal(...))` + `kit::header`/`body`/`footer`, menus with `kit::menu_panel`/`menu_row`. Tests drawing kit views must install a theme (`Catalog::load(...).install`) or token sizes resolve to 0. New `capture::OWN_MESSAGE`. M08: picker cell menus already use the kit; quick reactions draw through `ExpressionPicker::emoji_visual` (`pub(crate)`). M16: toasts follow the composer; the mockup's "Waiting for connection..." in the composer isn't built. M19: build settings on `kit::dialog` and the scrim; reuse `kit::switch`, `radio_row`, `checkbox`. M20: status menu entries in `profile/status_menu.rs::entry` (`status_menu_width` 240); `moderation/kit.rs` has `select`/`select_anchor`.
