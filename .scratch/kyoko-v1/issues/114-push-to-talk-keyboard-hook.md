# 114 — Push to talk like Discord: a keyboard hook, not a hotkey

**What to build:** Follow-up from 100. Push to talk registers a Windows hotkey (`RegisterHotKey` in `apps/desktop/src/integration.rs`), so while it is bound the key combination never reaches other apps: binding a game key steals it from the game. The user asked for it to work like Discord (2026-10-29): Discord's desktop app watches the keyboard with a low-level hook and lets every key through, so the key keeps working in the focused app while it also opens the mic. Replace the push-to-talk hotkey with a low-level keyboard and mouse hook (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`) on its own thread with a message loop, which always calls `CallNextHookEx` and never swallows input. It reports press and release of the bound combination (modifiers included, key repeat ignored) as today's `Request::PushToTalk(bool)`. Like Discord, mouse side buttons (Mouse 4/5) can be bound in the keybind recorder, and a "Push to Talk Release Delay" slider (0–2000 ms, Discord's default 20 ms) holds the mic open briefly after release. The hook runs only while Push to Talk mode is on and a binding exists, and is removed otherwise. Other global hotkeys (40) stay as they are.

**Source of the Discord behaviour:** for facts only, never copied. The settings page the desktop app shows (push-to-talk options, release delay range, default and setting names) is in the public web bundle the desktop app loads from discord.com: read it as earlier tickets did (no token, neutral User-Agent `kyoko-dev`, no identity) and cite what you found. The hook itself lives in Discord's native modules (compiled `.node` files, not JavaScript), which are not on this machine; the hook approach is taken from Discord's behaviour (the key still reaches other apps). Don't launch the installed Discord app.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Push to talk no longer uses `RegisterHotKey`; a low-level hook reports press and release of the bound keys and always passes the input on (`CallNextHookEx`, never a non-zero return)
- [ ] Key repeat while held sends no extra presses; releasing any key of the combination releases; losing the hook (the hook thread stops) releases the mic, never leaves it open
- [ ] Mouse 4 and Mouse 5 can be recorded as a push-to-talk binding and work through the mouse hook
- [ ] Release delay setting (`voice.push_to_talk_release_delay`, 0–2000 ms, default 20) on the Voice page under Push to Talk; session test: the mic stays open for the delay after release and closes after it; a press within the delay keeps it open
- [ ] The hook exists only in Push to Talk mode with a binding; switching to Voice Activity or clearing the binding removes it
- [ ] Hook callback does no work beyond matching and posting the event (Windows drops slow low-level hooks); matching logic tested without a real hook (fed key events, checks the pressed/released stream); in-process UI test for recording Mouse 4; settings capture checked by eye
- [ ] `docs/voice.md` and `docs/desktop.md` updated; 100's deviation note no longer applies
