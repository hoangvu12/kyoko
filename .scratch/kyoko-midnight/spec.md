# kyoko Midnight: Discord's layout in the Midnight theme

## Goal

Kyoko's look moves from a 1:1 port of oxidcord to a 1:1 of Discord's current desktop client, styled with refact0r's Midnight theme (Vencord flavour 2.1.1, MIT). Behaviour stays what the v1 tickets built; these tickets restyle it and add what Discord has and kyoko lacks around expressions, commands and the composer.

## Sources

- The spec: `docs/design/midnight.md` (palette, structure, every region, expressions and commands, sizes, motion).
- The mockups: `docs/design/mockups/*.html`, 101 pages of plain HTML and CSS at 1280 by 800, every value a Midnight variable in one `:root` block. `docs/design/mockups/index.html` shows them all as thumbnails, grouped by area. Open a page in a browser at 1280 by 800 to compare with a kyoko capture. Icons, avatars, banners and art in them are placeholders; kyoko uses its own icons and the gallery.
- Measurements marked "measured" in the spec come from the live Windows client at 100%. Pages without a capture behind them follow Discord from memory; their spacing is approximate.
- Design history: `.scratch/kyoko-design/handoff.md`.

## Decisions

- The window is floating panels on a darker frame, Midnight's palette and sizes become kyoko's theme tokens, and the Midnight variable block is what ticket v1 57's colour import maps onto.
- Unicode emoji are drawn from a bundled Twemoji set (`jdecked/twemoji`, graphics CC-BY 4.0) everywhere, not the system emoji font.
- The composer shows `+`, GIF, sticker, emoji and apps. No gift button, no send button (as Discord).
- Kyoko never shows Nitro upsells: what needs Nitro is shown locked with the reason.
- Kyoko supports all of Discord's built-in slash commands.
- Discord's GIF provider is Klipy; kyoko follows the web client's provider.
- Vencord's own UI (Spotify controls, member counts, translate button) is out of scope; kyoko's plugin and theme pages take Vencord's settings place.
- Voice and the soundboard stay out of scope; voice messages are played, not recorded.

## Relation to the v1 tickets

The v1 tickets in `.scratch/kyoko-v1/issues/` built kyoko to oxidcord's look and own the behaviour. A Midnight ticket restyles a screen after its v1 ticket has landed (its `Blocked by` names the v1 ticket as `v1 NN`). Whether open v1 UI tickets switch their look target to Midnight now is the orchestrator's and the user's call; until then they keep oxidcord's.

## Checking a ticket

- Behaviour changes in the session get behaviour tests at the session seam against mockcord (`AGENTS.md`).
- A restyled screen is compared with its mockup: a kyoko capture at 1280 by 800 beside the mockup rendered at 1280 by 800 (ticket 02 adds the tooling).
- `cargo xtask headless-run --mock` and the desktop app against mockcord, as always.
