# 83 — Server rail: Discord's hover tooltip and voice indicator

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Hovering a server icon in kyoko shows a plain label (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_6.png`). Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_5.png`) shows a tooltip card with the server name and, when people are in voice, a speaker icon with their avatars; the server icon also carries a speaker badge when a voice channel there is active.

**Blocked by:** none

**Status:** resolved

- [x] Hover tooltip card on the rail: server name, and the voice row (speaker icon plus up to a few avatars) when anyone is in voice
- [x] Speaker badge on a server icon with active voice, from voice states
- [x] Capture checked by eye; Midnight tokens only

## Answer

Merged to main as `3710491` (worker `e99ce2cf`, lane A; full verify re-run by the orchestrator: green apart from the old theme_links flake (fixed by 109, `xtask test` green after rebasing onto it) and one `headless-run` failure "redrawing the timeline asked ShowMeYourName again" that passed 3 of 3 reruns (ticket 64's display check; watch it); kyoko-only captures `server-rail-hover` and `server-rail` checked by eye against Image_5.png). Docs: `docs/ui.md` ("Server rail"), `docs/parity.md`.

- Root cause: the rail gave each server a text tooltip only, and the rail projection held no voice data.
- `bridge::Rail` lists who is in each server's viewable voice channels (`RailGuild::voice`, user ids sorted by display name) from guild voice states, and the rail now updates on voice state changes. The host publishes `voice` (`{key, avatar}`, at most 4) and `voice_more` per server and loads the avatars with the icons.
- Luau: zgx's `tooltip` accepts a node; `rail_tile` takes a node `tooltip`, `tooltip_placement` and a `speaker` flag. The card shows the name and, with anyone in voice, a speaker, up to four avatars and "+N"; folders get a name-only card. A speaker badge sits top-right on a server with active voice. Midnight tokens only.
- Test: `rail::the_rail_follows_who_is_in_voice_in_each_server` (bridge, against mockcord). Capture `server-rail-hover` points at (45, 155), the first server in the open mock folder.
- Deviations: closed folders get no badge or voice row; Discord's small community icon before the name isn't drawn; avatar order is by name (Discord's order unknown).
- For later: a live or stage indicator can build on `RailGuild::voice`. Captures show black square corners outside zgx popup tooltips (all tooltips; a capture compositing artefact).
