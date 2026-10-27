# 86 — Blinking caret and the right mouse cursors

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). The text caret in inputs doesn't blink. Hovering buttons doesn't show the hand cursor; the user wants every cursor shown the way a browser does (pointer on clickable things, text I-beam over inputs and selectable text, resize cursors on splitters, not-allowed on disabled controls, grab where something drags).

**Blocked by:** none

**Status:** claimed

- [ ] The caret blinks at the platform rate (Windows caret blink time; solid while typing, resumes blinking after a pause; no blink when the window loses focus) in every text input
- [ ] Cursor rule written in `docs/ui.md`; every clickable kit component (buttons, links, list rows, tabs, menu items, icons with actions) shows the pointer, inputs show the I-beam, disabled controls the default or not-allowed cursor, splitters the resize cursor
- [ ] If zgx lacks caret blink or per-element cursors, add the smallest support needed (zgx is vendored/pinned: say where it lives and what changed) and note it; in-process UI test for at least the button cursor and caret blink state
