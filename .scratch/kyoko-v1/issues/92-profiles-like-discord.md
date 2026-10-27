# 92 — Profiles like Discord: DM panel, full profile, popout menu

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Three profile surfaces differ from Discord:
- DM side panel: Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_7.png`) shows banner, avatar with status, name, username and badges, mutual servers row with icons, Member Since, View Full Profile at the bottom; kyoko (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_8.png`) shows a different layout.
- Full profile: Discord, someone else's (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_11.png`): profile theme colours and effect, avatar decoration, name, username, server tag and badges, Add Friend / Message / more, bio, Member Since (Discord and this server), Roles, Connections with details, Note, and tabs Activity / Mutual Friends / Mutual Servers on the right. Own profile (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_9.png`) adds editing (not in scope: kyoko shows the others' layout for its own user, minus Add Friend). kyoko today: `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_10.png`.
- Popout "more" (three dots) button does nothing (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_12.png`); Discord's menu (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_13.png`): View Full Profile, Share Profile (copy link), Invite to Server (submenu), Ignore, Block, Report User Profile (open in browser), Copy User ID. Leave out Shop This Look.

**Blocked by:** 77, 81

**Status:** resolved

- [x] DM side panel matches Discord's layout
- [x] Full profile matches Discord's (others' layout), with the three tabs; profile theme colours (`theme_colors`) and the profile effect at least as a static gradient
- [x] Popout more menu with the items listed (destructive ones through kyoko's existing confirmations; Ignore if the API is known, else left out and noted)
- [x] The popout's "Message @user" quick input where Discord has it
- [x] Captures checked by eye against the screenshots; Midnight tokens for kyoko's own surfaces

## Answer

Merged to main as `456a244`, `e6601aa`, `5efa629` (worker `b4d1a393`, lane A; checks re-run by the orchestrator: all green, `xtask test` needed one rerun for a flake in `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus` that passed in the workspace run and on rerun; parity profile-popout 20.8%, full-profile 41.2% (Discord's larger layout), dm-sidebar 28.8%; kyoko-only `profile-menu` and `server-profile` checked by eye).

- Root cause of the dead More button: zgx treats anything chained after `.tooltip()` as tooltip styling, so `.on_click()` after it never fired. Also fixed in popout Add Friend, full-profile close, custom status Clear, pin popout Unpin, inbox buttons and apps launcher tiles. Rule in `docs/ui.md`: `.tooltip()` goes last.
- DM column: banner with round Add Friend and More, avatar with status, name, handle and server tag, mutual server icons and mutual friends, placeholder bars while loading, bio, Member Since, View Full Profile.
- Full profile: card on the left (banner, avatar, names, Add Friend / Message / More, bio, Member Since, Roles, Connections), tabs Activity / Mutual Friends / Mutual Servers on the right. Own profile has no Add Friend.
- `theme_colors` and `profile_effect` read from the global and the server profile (server wins); a theme paints popout, full-profile card and DM column as a primary-to-accent gradient; the effect is a static glow.
- Profile menu (popout, full profile, DM column): View Full Profile, Share Profile (`https://discord.com/users/<id>`), Invite to Server submenu (creates a 7-day invite and sends it in the DM), Ignore/Unignore (`PUT`/`DELETE /users/@me/relationships/{id}/ignore`, `user_ignored`, docs.discord.food), Block via the existing confirmation, Report (support form in the browser), Copy User ID.
- Popout "Message @user" is a real input: Enter sends (opening the DM if needed) and switches to it.
- Not done: Note (kyoko keeps no notes), avatar decorations, connection details and brand icons, Member Since icons, animated effects, blurred banner background; Ignore has no explanation dialog (toast instead).
- For later: the popout and full profile rebuild on every card change, which resets a half-typed quick message and closes an open menu (the floating menu is guarded, the popout isn't). `show_card` takes the card with its menu. Mock profile themes are keyed on the id's timestamp part (`docs/parity.md`).
