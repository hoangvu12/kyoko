# 16 — Composer restyle and extras

**What to build:** Restyle the composer to Midnight (47 high, `--bg-3`, 1-point `--hover` border) with `+` and GIF, sticker, emoji and apps buttons (no gift, no send). Add Discord's live markdown inside the field (bold, italic, inline code, spoiler, quote, mentions and channels as chips, custom emoji as images, markers dimmed), the grow-to-half-the-chat rule, the reply bar with the @ toggle, the character counter (under 200 left, red past the limit), the upload tray (216 cards, spoiler, edit and remove on hover), the drop overlay ("Upload to #channel", Shift uploads directly), upload progress and failure (Retry, Remove), the too-big notice, and the `+` menu (Upload a File, Create Thread, Create Poll, Use Apps).

**Blocked by:** 01, 03; v1 27

**Status:** claimed

**Mockups:** `docs/design/mockups/composer-markdown.html`, `docs/design/mockups/composer-markdown-over-limit.html`, `docs/design/mockups/composer-uploads.html`, `docs/design/mockups/composer-drop.html`, `docs/design/mockups/composer-upload-failed.html`, `docs/design/mockups/composer-plus-menu.html`

**Spec:** `docs/design/midnight.md`, Chat panel, Composer extras, Composer buttons

- [ ] Each state matches its mockup
- [ ] Markdown in the field does not change the sent text; IME and spellcheck still work
- [ ] Upload progress and spoilered uploads are covered by session-seam tests
- [ ] Shift on drop uploads directly
