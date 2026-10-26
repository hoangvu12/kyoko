# 05 — Restyle the timeline

**What to build:** Restyle the message list to Midnight: rounded message cards with `--message-hover`, the mention and reply-to-you gradients with their 4-point bars, the unread divider and the floating new-messages bar, embeds (radius, 4-point colour bar), code blocks on `--bg-3`, attachments, the hover bar, deleted and edited styling. Add what the mockups show around expressions: GIFs from the GIF provider shown as the video alone (no embed card) with the favourite star on hover and a GIF pill when autoplay is off, stickers at 160, jumbo emoji at 48, the emoji popout (custom: server and Join; Unicode: "A default emoji...") and the sticker popout, placed beside what was clicked and kept inside the chat. A sent link's embed can be removed by its author (Remove all embeds, the `SUPPRESS_EMBEDS` flag).

**Blocked by:** 01, 03, 04; v1 25, 26, 28

**Status:** claimed

**Mockups:** `docs/design/mockups/midnight-discord.html`, `docs/design/mockups/expressions-in-messages.html`, `docs/design/mockups/expressions-in-messages-emoji-popout.html`, `docs/design/mockups/expressions-in-messages-sticker-popout.html`, `docs/design/mockups/link-preview-while-typing.html`

**Spec:** `docs/design/midnight.md`, Chat panel, Expressions and commands

- [ ] The timeline matches its mockups in captures (02)
- [ ] GIF favourites toggle from the timeline and appear in the GIF picker
- [ ] Removing embeds is a session command with a behaviour test against mockcord
- [ ] Popouts never cover the item they opened from and stay inside the chat
