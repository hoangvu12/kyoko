# 75 — Slash command options: crash, caret gap, options not shown

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Typing a slash command (`/dl` from a bot, two optional options) shows the caret far to the right of the command name (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_9.png`), the options list only appears after clicking "+2 optional" (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_10.png`), and **picking an option crashed kyoko**. Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_11.png`) shows the options list at once when a command with options is chosen, and puts a dim "+2 options" hint right after the caret inside the composer.

**Blocked by:** none

**Status:** needs-info

- [ ] Root cause of the crash found and fixed; a test that picks an optional option of a command (mockcord command with only optional options, string type) and fails without the fix
- [x] The caret sits right after `/dl ` (no gap); the composer's text layout is correct with the command chip
- [x] Choosing a command that has options opens the options list right away (required first, then optional), as Discord does; the "+N options" hint follows the caret
- [x] Picking an option inserts its chip and moves the caret into its value; Esc and the close button still work
- [x] Parity against the Midnight command-picker mockups where one exists; kyoko-only capture otherwise
- [x] The command list and options list float over the messages as a popover (Discord, `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_1.png`); today they take layout space and push the messages up (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_2.png`)

## Progress (merged, crash still open)

Merged to main as `cc902ae` (worker `98d16829`, lane B; checks re-run by the orchestrator: all green; parity command-picker 11.7%, command-options-optional 12.6%, pair checked by eye).

- Caret gap: zgx's `ui.each` grows even with `grow = 0`; the chip list is wrapped in a non-growing row, the tail input is 2 points wide, a filler after it focuses the input. The "+N options" hint sits after the caret and toggles the list; the hint bar's "+N optional" button is gone, the list header says "OPTIONS".
- Commands with no required options open the options list at once (required first, then optional); commands with required options still focus the first required chip.
- Popovers: on the main composer the command picker, options, choices, file drop and suggestions float over the messages. The Rust timeline keeps its size but is clipped at the popover's top (new `composer_float` slot via `frame.slot`; `timeline_slot` in `ui.rs` takes one more parameter). zgx's `ui.popover` isn't used (a separate popup window on Windows that takes keyboard focus). The thread composer keeps in-flow lists. `docs/ui.md` "Composer popovers".
- Mockbot `/lookup [appid] [query]` (two optional strings) in mockcord; bridge test `a_command_with_only_optional_options_lists_them_at_once_and_runs_the_one_picked`; kyoko-only screen `command-options-only-optional`.
- Crash: not reproduced. Mouse, keyboard, typed `/lookup `, an em dash in descriptions, autocomplete, the user's seven plugins, about 1,500 random steps: no crash. No Windows crash record; the crashing run had no stderr capture. Needs the user to repeat `/dl` with stderr captured (`RUST_BACKTRACE=1`), and the real option types.
- User's answer (2026-10-28): it was a crash, not a close; they don't remember the time. Probably picked the option with the keyboard while typing (they usually stay on the keyboard). Next real run captures stderr with `RUST_BACKTRACE=1`; a keyboard-only fuzz of option picking while typing is the next thing to try on mock.

