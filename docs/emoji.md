# Emoji

Kyoko draws Unicode emoji from a bundled Twemoji set (`jdecked/twemoji`, graphics CC-BY 4.0), as Discord draws them from its own set, never from the system emoji font. Custom emoji are images from the CDN through the gallery (`docs/media.md`). No network is used for a Unicode emoji, ever.

## The set

`crates/twemoji` (package `kyoko-twemoji`, no UI dependency, depends only on zstd) holds `assets/twemoji.pack`: the 4,009 SVG files of `assets/svg/` in the jdecked/twemoji v17.0.3 release, unchanged. The pack is the files sorted by name, cut into blocks of about 512 KiB, each block an independent zstd frame (level 19), and an index of `name, block, offset, length`, itself a zstd frame. It is compiled into the binary with `include_bytes!`.

- Pinned release: `twemoji::RELEASE` (`v17.0.3`), `RELEASE_URL` (the GitHub tag tarball) and `RELEASE_SHA256` (`a0855654b633045ae2337537e77f1bb4361162f7fcd910e613eaab1d6d9c5fca`).
- `cargo xtask twemoji` fetches the tarball with curl (or takes `--archive <file>` offline), refuses it unless its SHA-256 matches, reads `assets/svg/*.svg` and writes the pack. The same tarball always gives the same pack, byte for byte. To move to a newer release, change the three constants, run the command, and update `THIRD_PARTY_NOTICES.md` and the test of emoji the release lacks.
- Sizes: the SVGs are 10,121,593 bytes; the pack is 1,298,833 bytes in the repository and adds 1,298,432 bytes to `kyoko.exe` (zstd is already linked for HTTP). Larger blocks pack smaller (1.14 MB at 2 MiB blocks) but decompress more per lookup.

## Finding an emoji's file

Twemoji names a file after the code points of the sequence in lowercase hex joined by `-`, with U+FE0F dropped except in some ZWJ sequences that keep it (`2764-fe0f-200d-1f525`, but `1f441-200d-1f5e8`). Kyoko keys every file by its code points without U+FE0F, so either spelling of a sequence finds it:

- `twemoji::file(sequence)`: the file for one sequence (skin tones `1f44b-1f3fd`, ZWJ families `1f468-200d-1f469-200d-1f467-200d-1f466`, flags `1f1fa-1f1f8`, subdivision flags `1f3f4-e0067-e0062-e0065-e006e-e0067-e007f`, keycaps `23-20e3` with or without U+FE0F).
- `twemoji::split(text)`: the text as `Piece::Text` and `Piece::Emoji { text, file }`, taking the longest sequence the set has at each place, with the variation selectors around it. A sequence the set lacks falls back to its parts (a family Twemoji does not draw shows its people), and whatever has no file stays text in the system emoji font. `©`, `®` and `™` are emoji only with U+FE0F, and U+FE0E (text presentation) keeps any character as text.
- `twemoji::svg(file)`: the SVG bytes, decompressing the file's block. The last block is kept, so a picker page of neighbouring emoji decompresses it once.

Every emoji in the shortcode table (`protocol::expressions::unicode`) resolves, with each of the five skin tones where the table allows them, except the nine newer emoji the table carries that Twemoji 17.0.3 does not draw yet (`cracking_face`, `leftwards_thumb_sign`, `rightwards_thumb_sign`, `monarch_butterfly`, `pickle`, `lighthouse`, `meteor`, `eraser`, `net_with_handle`); they stay in the system font. `crates/twemoji/tests/twemoji.rs` walks the table and fails on any other miss. The picker's skin tone puts the tone on every person of a hand-holding sequence (`1f9d1-1f3fb-200d-1f91d-200d-1f9d1-1f3fb`), as Unicode's list of sequences does.

## Drawing

The desktop never rasterises emoji itself. `apps/desktop/src/emoji.rs` turns a file into a zgx `SvgData` once per process (`emoji::art`), and zgx's own SVG renderer (resvg) rasterises it at the node's size times the window scale, so emoji are crisp at 150%, and keeps the rasters in its SVG cache (32 MiB and 1024 rasters at most, shared by every copy of the same emoji at the same size). `emoji::glyph(text, side)` is the emoji's SVG, or the text in the system font when the set lacks it.

- Messages (`timeline/body.rs`): every text run, mention label and link label is split; each emoji becomes a placeholder span with the image laid over it, the same path as custom emoji (`timeline/inline.rs`). The placeholder's text is what copying it gives: the emoji's own characters, or `<:name:id>` (`<a:name:id>` animated) for a custom emoji, set at 1 point with negative tracking in a transparent colour, padded to the emoji's side plus a point either side. Inline emoji are 22 points, jumbo 48. Wrapping is the text's own: a Unicode emoji breaks as its characters do, a custom emoji's markup holds to its neighbours as the word joiner did. Inside an unrevealed spoiler no image is laid over the placeholder. Inline code keeps emoji as text.
- Copying: zgx's selection copies the placeholder text, so Ctrl+C and the selection's Copy give the emoji and the `<:name:id>` markup, in plain text and HTML. Before, a custom emoji was dropped. `timeline/emoji_tests.rs` selects a rendered message and checks both.
- Reactions: 16 points (`timeline/extras.rs`).
- Expression picker (`pickers/emoji.rs`): cells (40 points in 48-point cells), the footer (28), the skin tone button (24) and the search field's hovered `:name:` placeholder as text.
- Custom status: the dialog's emoji button (24) and the status line of the profile popout and full profile (16). Onboarding options (24).
- Luau: the composer's emoji suggestions (20 in the 24-point slot) and the user panel's status emoji get the SVG source as `svg` and `emoji_svg` in their shared values, drawn with `ui.svg { code = ... }`.

No request reaches the network for a Unicode emoji: `timeline/emoji_tests.rs` lists a message's images the way the timeline does, lets mockcord serve them and checks that only the custom emoji and the avatar were fetched.

## Memory

The index takes about 0.76 MB once the first emoji is looked up (built in about 5 ms), plus the last block, at most 512 KiB. Each emoji shown keeps its SVG source (450 bytes to a few KiB; 2.5 KiB on average), and zgx keeps its rasters: 4.4 KB for an inline emoji at 150%, 20.7 KB for a jumbo one, 9.2 KB for a picker cell, within zgx's SVG cache. Decompressing all 4,009 files takes about 17 ms in a release build, so opening any picker section is not noticeable.

## Where emoji are still system text

Text in one Luau or Rust label (`ui.text`, `text(...)`) cannot hold an image yet. Luau's `ui.rich` spans take an SVG icon (`icon = { code = ... }`), which a later ticket can use for single-line labels once truncation with inline icons is checked. Until then these show the system emoji font:

- Names: server names (rail tooltips, guild header), channel and thread names (sidebar, top bar, thread panel, threads popout, forum), member and DM names, author names in the timeline, friends, profiles and the quick switcher.
- Statuses and activities as text: the member list's activity line, the friends subtitle, DM rows.
- Plain-text snippets: reply previews, search results, pins, forum excerpts, forward previews.
- Embed titles, author names and field names, poll questions and answers (an answer's emoji is not drawn at all), forum tag labels, the picker's skin tone menu and its footer label.
- The composer's text field: a zgx text area holds text only.
