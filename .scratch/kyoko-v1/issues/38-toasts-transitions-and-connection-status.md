# 38 — Toasts, transitions and connection status

**What to build:** Toasts, view transitions and a connection status indicator behave as in oxidcord.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] Toasts and transitions match oxidcord in the parity captures
- [x] The connection status shows live, reconnecting and offline states

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `fff6090`. Docs: `docs/ui.md`, `docs/parity.md`, `docs/themes.md`, `THIRD_PARTY_NOTICES.md`.

- In-window toasts (separate from 40's Windows toasts): a Rust root layer (`crate::toasts`, in `layered(...)` and `src/ui/root.rs`, 0x0 when empty) drawn like oxidcord's: 382 pt wide, 50 pt from the top and 16 from the right, kind icon, close on hover, 5 s each, at most three, the same text restarts its timer, hover pauses. Session rejections become error toasts unless a surface that just acted shows them (28's timeline/thread action bar, or a composer that sent within 15 s; the composer now shows only rejections within 15 s of its own send). Copy actions confirm with a toast ("Link copied", "ID copied", "Text copied", "Invite link copied") or an error if the clipboard refuses. 45's plugin notice stays the red bar. API: `toasts::show(ToastKind, text)`, `toasts::copy(text, label)`; no new ad-hoc notice bars for transient feedback.
- Connection banner: oxidcord's 32 pt bar above the timeline with an outlined Retry. Live: none; "Connecting..." on first connect (no Retry); "Reconnecting in 3s..." while 07's backoff waits, "Reconnecting..." during an attempt; offline (red) after 3 failed attempts in a row, "You're offline. Retrying in Ns..."; stopped (gateway gave up, e.g. 4014): "Disconnected from Discord: …", no Retry; signed out: 21's login screen. Retry sends the existing `Command::Reconnect`, which only hangs up an open socket and can't skip 07's backoff. `Link::reset_connection()` runs on every session start.
- Transitions: Rust dialogs slide down 80 pt and fade in over 250 ms, the expression picker drops 8 pt and fades over 150 ms, toasts slide and fade 400 ms in / 200 ms out; reduced motion (37's setting or the system's) keeps only fades; the Luau rail morph, unread pill and search widening respect it too (read when built). New dialogs and popovers use `motion::modal` / `motion::popover`. zgx closes modals instantly and draws its own backdrop, so no exit animation or backdrop fade; zgx menus and 30/33's custom popouts don't animate. Toasts stack as a plain column (oxidcord's collapsed stack not reproduced).
- Theme token `banner_button`; Lucide icons `circle_x`, `circle_check`, `info` (125 icons).
- Tests: `the_connection_status_follows_a_drop_an_offline_period_and_the_reconnect` (mockcord drop and 4 refused reconnects: live → reconnecting → offline → live), `a_stopped_gateway_shows_an_offline_banner_without_retry`, `a_refused_command_becomes_one_error_toast_that_expires` (`crates/bridge/tests/bridge/toasts.rs`); `ui::root` now covers 10 layers; two banner screens in the load check. Checks re-run by the orchestrator in lane C.
- Parity (port 8493, 150% scale): toast 6.3%, connection-offline 6.2% whole window (banner 3.4%). The `connection-offline` capture sets the banner directly, as oxidcord's step fakes its state.
- For 41: keep `Link::reset_connection()` on the account-switch path; give Switch Accounts feedback through toasts. For 51: in-window toasts never touch `allow_notification` or Windows notifications. For 54: a refusal from `mark_all_read()` shows as an error toast unless claimed; a success toast via `toasts::show(ToastKind::Success, …)` from the host side.
