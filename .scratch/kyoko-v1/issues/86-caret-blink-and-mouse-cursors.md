# 86 — Blinking caret and the right mouse cursors

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). The text caret in inputs doesn't blink. Hovering buttons doesn't show the hand cursor; the user wants every cursor shown the way a browser does (pointer on clickable things, text I-beam over inputs and selectable text, resize cursors on splitters, not-allowed on disabled controls, grab where something drags).

**Blocked by:** none

**Status:** resolved

- [x] The caret blinks at the platform rate (Windows caret blink time; solid while typing, resumes blinking after a pause; no blink when the window loses focus) in every text input
- [x] Cursor rule written in `docs/ui.md`; every clickable kit component (buttons, links, list rows, tabs, menu items, icons with actions) shows the pointer, inputs show the I-beam, disabled controls the default or not-allowed cursor, splitters the resize cursor
- [x] If zgx lacks caret blink or per-element cursors, add the smallest support needed (zgx is vendored/pinned: say where it lives and what changed) and note it; in-process UI test for at least the button cursor and caret blink state

## Answer

Merged to main (worker `0efcae06`, lane B, commit "Blink the caret and stop layers hiding cursors"; checks re-run by the orchestrator: all green, parity app-frame 14.2%, toast 14.9%). Docs: `docs/ui.md` ("Cursors and the caret"), `docs/spike.md` (two zgx findings). zgx unchanged (`fc7d0f1`, no patch).

- Cursors: most controls already asked for the hand, but zgx picks the cursor from the topmost node under the pointer and does not skip `pointer_events = "none"` (clicks do). Two invisible window-sized nodes sat on top at all times: the settings dialogs' size-measuring box (`settings/dialog.luau`, now invisible while closed) and the toast layer's container (`toasts.rs`, now mounted only while a toast shows; the countdown resumes when the list empties, so a toast removed while hovered can't leave the next one paused).
- Caret: `apps/desktop/src/caret.rs` blinks the focused field's caret at `GetCaretBlinkTime` (solid if blinking is off in Windows), solid while typing, hidden while the window is in the background (zgx), run from the root so rebuilds don't cancel it; no blink in parity captures.
- Tests `apps/desktop/src/ui/cursors.rs`: `controls_show_the_hand_fields_the_i_beam_and_no_closed_layer_hides_either` (real shell under every root layer; fails with either fix reverted) and `the_caret_blinks_holds_while_typing_and_stays_hidden_while_the_window_is_away`.
- For later: any full-window layer must be zero-size, invisible or unmounted while empty, or it hides every cursor under it. A button whose child has its own handler needs an explicit cursor (zgx's automatic hand only looks at the innermost node). Upstream zgx fix worth reporting (user's call): `Scene::cursor_at` should skip `pointer_events = "none"`.

