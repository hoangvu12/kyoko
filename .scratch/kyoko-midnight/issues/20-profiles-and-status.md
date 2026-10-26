# 20 — Profiles and status

**What to build:** Restyle the profile popout, the full profile dialog (banner, avatar, names, tags, badges, Message, Bio, Member Since, Connections, Note, the Board / Activity / Wishlist tabs), the status menu and the custom status dialog to the mockups.

**Blocked by:** 01, 07; v1 30

**Status:** resolved

**Mockups:** `docs/design/mockups/profile-popout.html`, `docs/design/mockups/profile-full.html`, `docs/design/mockups/status-menu.html`, `docs/design/mockups/dialog-custom-status.html`

**Spec:** `docs/design/midnight.md`, Settings, Other surfaces

- [x] Matches the mockups
- [x] Missing profile parts degrade as today

## Answer

Merged to main as `4fa9d30`. Docs: `docs/ui.md`, `docs/parity.md`.

- Popout (`profile/popout.rs`): 300 wide, `bg_3`, `radius_lg` outer corners, 1-point border; banner falls back image, accent colour, `bg_2`; round Add Friend and More buttons on the banner (More: View Full Profile, Block, Copy User ID); 80-point avatar ring (click opens the full profile); names in `accent_1`, handle, pronouns, guild tag chip, badges, custom status; a `bg_4` inner card (`profile_inner_radius`) with activities and About Me, then Member Since, roles and mutual counts; a "Message @name" field at the bottom (opens the DM).
- Full profile (`full.rs`): 900 by 640 on the shared scrim, round close button, 120-point avatar; left column with names, Message, Add Friend, More, Bio, Member Since ("Jan 1, 2022 · Server: Oct 15, 2025"), Roles, Connections cards (green check when verified); right column tabs only where there is data (Activity cards, Mutual Friends, Mutual Servers), else "Nothing to show here yet."
- Status menu (`status_menu.rs`) is Discord's account popout above the user panel: banner, avatar, names, a `bg_4` menu (Edit Profile, current status, Set Custom Status, Switch Accounts, Copy User ID); status submenu with Discord's descriptions; Switch Accounts opens v1 41's accounts view as a submenu (still works, account-switcher capture checked). **Log Out was removed from the status menu**; it lives in settings only.
- Custom status (`custom_status.rs`): avatar with live preview bubble, "What's cookin', <name>?" field, Clear after and Status selects (`moderation::kit::select`, now `pub(crate)`), Cancel and Save (Save also sends `SetStatus` when changed).
- Bridge: `ProfileCard` gained `GuildTag` (from `primary_guild`, else `clan`), `ProfileConnection` (verified flag), `guild_name` (falls back to the profile's own `guild_member`); `StatusChoice::description()`/`key()`; banner and accent helpers on `PanelUser`. Bridge seam test for tag, connections, server name and the clean fallbacks. The DM profile column shows the same tag chip (`dms.profile.tag`, `tag_badge`). 13 derived tokens; `user_plus`, `smile`, `refresh` added to the Rust icon table. Mockcord gives users whose id is a multiple of 3 a "RUST" guild tag.
- Rebased by the orchestrator onto v1 63: the More menu's Block closes the profile and sends `profile::Request::Block`, which opens 63's confirmation dialog.
- Checks re-run by the orchestrator in lane C: all green. Mockup parity (port 8493): profile-popout 20.2% (21.2% before), full-profile 27.6% (41.9%), status-menu 18.2% (18.7%), custom-status 13.1% (16.3%).
- Left out (no data or action): Board and Wishlist tabs, Note, the popout's "+ add role", game art and durations on activity cards, profile theme colours (names stay `accent_1`), Idle/DnD duration submenus, brand icons for connections (initials instead). The popout keeps its click anchor; the status submenu is bottom-aligned with the popout.
- For later tickets: M19: Edit Profile opens the settings Profiles page via `Request::EditProfile` (`open_settings("profiles")`); keep Log Out in settings. `profile/parts.rs` has `words(text, size)`, `size(runtime, token)` (a token's number for absolute positions), `banner`, `avatar` with `Badge`, `avatar_ring`, `tag_chip`, `round_button`, `icon_button`; a Profiles preview could reuse `popout::card`. M23: Member Since reads `ProfileCard::guild_name`. zgx `top`/`left`/`size()` take numbers, not tokens (use `parts::size` or `.w(tok).h(tok)`). Hover-opened submenus must guard their signal (`show_submenu`) or a rebuilt node's `PointerEnter` loops.
