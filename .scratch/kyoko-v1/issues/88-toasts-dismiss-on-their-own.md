# 88 — Toasts disappear on their own

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Toasts stay on screen until the mouse hovers them; they should time out by themselves (hover pauses the timer, as in Discord).

**Blocked by:** none

**Status:** claimed

- [ ] Root cause found (timer only started or advanced on hover or on input events, or the redraw isn't scheduled); fixed so a toast times out with no input at all
- [ ] Hover pauses, leaving resumes; error toasts may stay longer as documented in `docs/ui.md`
- [ ] Test that drives time without any input events and sees the toast go
