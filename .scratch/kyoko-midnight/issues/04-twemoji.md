# 04 — Bundle Twemoji and draw Unicode emoji with it

**What to build:** Unicode emoji are drawn from a bundled Twemoji set (`jdecked/twemoji`, graphics CC-BY 4.0) wherever kyoko shows them: message text (inline 22, jumbo 48), reactions, pickers, autocomplete, names, statuses, channel names, embeds. Pack the set compactly (for example the SVGs in one compressed archive rasterised on demand at the sizes needed, or pre-rasterised atlases), resolve sequences (skin tones, ZWJ, flags, keycaps) to files, and draw them through the same path as custom emoji so wrapping, selection and copy stay as they are (copying yields the Unicode text). Keep the system emoji font as the fallback for anything the set lacks. Check crates.io before adding a rasteriser.

**Blocked by:** none (v1 18 media pipeline and v1 26 inline custom emoji exist)

**Status:** claimed

**Mockups:** `docs/design/mockups/expressions-in-messages.html`, `docs/design/mockups/reactions.html`, `docs/design/mockups/emoji-picker.html`, `docs/design/mockups/midnight-discord.html`

**Spec:** `docs/design/midnight.md`, Unicode emoji

- [ ] Every emoji in the gemoji table resolves to a Twemoji image, sequences included
- [ ] Text with emoji wraps, selects and copies as before
- [ ] The bundle size is measured and stated in the answer; attribution is in `THIRD_PARTY_NOTICES.md`
- [ ] No network is used to fetch emoji
