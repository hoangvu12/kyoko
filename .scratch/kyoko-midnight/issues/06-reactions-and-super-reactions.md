# 06 — Reactions, super reactions and the reactions list

**What to build:** Reaction chips in Midnight's tokens (24 high, 2-point borders, reacted `--accent-2`), super reactions (burst colours as the chip's gradient border, a sparkle after the count, burst-colour fill when it is yours), the hover tooltip ("hungry, edelweiß and 2 others reacted with :thumbsup:"), the add-reaction and add-super-reaction buttons after the last chip, and the Reactions list dialog (Reactions and Super Reactions tabs, reaction column, who reacted, remove for moderators). Adding a super reaction is offered only to accounts that can send one; kyoko never shows an upsell.

**Blocked by:** 05

**Status:** resolved

**Mockups:** `docs/design/mockups/reactions.html`, `docs/design/mockups/reactions-list.html`

**Spec:** `docs/design/midnight.md`, Palette, Expressions and commands

- [x] Chips, tooltip and list match the mockups
- [x] Super reactions are read from `burst`, `me_burst` and `burst_colors` and toggle with the burst type
- [x] The reactions list loads who reacted, paged, and removing one is gated on Manage Messages
- [x] Session-seam tests cover super reactions and the list against mockcord

## Answer

Merged to main as `745af24`. Docs: `docs/ui.md`, `docs/rest.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md` (icons `sparkle`, `add_reaction`, `add_super_reaction`).

- Chips (`apps/desktop/src/timeline/reactions.rs`): 24 high, 2-point borders, `radius_sm`; reacted `accent_2` at 20% with an `accent_2` border and `text_1`. Super chips: gradient border from `burst_colors` (else `purple_2` → `blue_2`), a sparkle after the count, a 20% gradient fill when yours. Add-reaction and add-super-reaction after the last chip on a hovered row; add-super only for Nitro accounts (no burst-credit count exists in the client), never an upsell. Tooltip "hungry, edelweiß and 2 others reacted with :thumbsup:", names loaded on hover.
- Reactions list: a new "View Reactions" menu item opens a `kit::dialog` over the scrim with Reactions and Super Reactions tabs, the reaction column, a paged list of who reacted (avatar, display name, username) and a remove button on hover for moderators. Wired for the channel timeline and the thread pane. Shared bridge model in `crates/bridge/src/reactions.rs` (`reaction_chips`, `reaction_tip`, `emoji_label`, `ReactionsList`).
- Session: gateway events read `type` (falling back to `burst`) and `burst_colors` leniently (bad colour elements dropped); burst counts kept apart (`count_details.burst`, `me_burst`). Add `PUT …/@me?type=1`, remove own `DELETE …/{emoji}/{0|1}/@me`; list `GET …?limit=100&after=…&type=0|1`, pages per message, emoji and type in `State::reactors` (replaces `State::reaction_users`), kept current by add/remove events; `RemoveUserReaction` (`DELETE …/{emoji}/{type}/{user}`) needs Manage Messages in a server channel and goes through the paced server-write queue. `AddReaction`/`RemoveReaction` carry `burst`; `LoadReactionUsers` carries `burst` and `after`. Routes checked on docs.discord.food.
- Mockcord: per-user reactor lists, the typed routes, `Control::react_as`/`unreact_as`; a new `-# showcase: reactions` section at the end of `#showcase` (so `timeline-system` now shows its start on both parity sides).
- Tests (`tests/session/reactions.rs`): super add/remove with the burst type; gateway burst and normal counted apart with a bad colour dropped; no Nitro → no request; list paged over 101 reactors with exact queries for both types; remove refused without Manage Messages (no request), with it one request on the burst route and the list updates. The reaction picker now saves favourites through `SetEmojiFavorite` (still titled "Add Reaction" in super mode). Checks re-run by the orchestrator in lane C: all green.
- Mockup parity (port 8493): reactions 14.5%, reactions-list 11.2% (new, mapped); timeline 17.6%. By eye chips, gradients, sparkles, add buttons, tooltip, tabs, column, 44-high rows and hover remove line up.
- Deviations: tooltips have no pointer arrow; super chips have no emoji glow; custom emoji in the tooltip (32) and list (20) scale the 16-point chip image. Capture helpers `TimelineView::open_reactions`, `reaction_anchor`, `reactions_settled`.
