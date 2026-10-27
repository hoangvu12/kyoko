# 88 — Toasts disappear on their own

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Toasts stay on screen until the mouse hovers them; they should time out by themselves (hover pauses the timer, as in Discord).

**Blocked by:** none

**Status:** resolved

- [x] Root cause found (timer only started or advanced on hover or on input events, or the redraw isn't scheduled); fixed so a toast times out with no input at all
- [x] Hover pauses, leaving resumes; error toasts may stay longer as documented in `docs/ui.md`
- [x] Test that drives time without any input events and sees the toast go

## Answer

Merged to main as `04f244a` (worker `e730df9f`, lane B; checks re-run by the orchestrator: all green; after rebasing onto 79, clippy and the desktop `ui::` tests re-run green). Docs: `docs/ui.md` ("Toasts, transitions and connection status").

- Root cause: the countdown was a task owned by the toast stack, and zgx cancels a component's tasks when it rebuilds; every new toast rebuilds the stack (so it draws above open dialogs), killing its own countdown, which only restarted on pointer enter/leave. Not a redraw problem: zgx timers wake the window by themselves.
- Fix (`apps/desktop/src/toasts.rs`): the countdown belongs to the layer's outer component, which never rebuilds, and starts when it first appears. Hover pauses, leaving resumes; every toast, errors included, lasts 5 s.
- Test `ui::root::a_toast_leaves_on_its_own_without_input_and_hovering_holds_it` (no input events; fails on the old code). `bridge::Toasts::lasting` / `ToastLayer::lasting` set the lifetime for tests.
- For later: any background task started inside a part of the screen that rebuilds is cancelled with it; start it from a component that stays mounted. The `toast` parity capture shoots about 1.5 s after the toast appears; keep it under 5 s.

