# 04 — Bundle Twemoji and draw Unicode emoji with it

**What to build:** Unicode emoji are drawn from a bundled Twemoji set (`jdecked/twemoji`, graphics CC-BY 4.0) wherever kyoko shows them: message text (inline 22, jumbo 48), reactions, pickers, autocomplete, names, statuses, channel names, embeds. Pack the set compactly (for example the SVGs in one compressed archive rasterised on demand at the sizes needed, or pre-rasterised atlases), resolve sequences (skin tones, ZWJ, flags, keycaps) to files, and draw them through the same path as custom emoji so wrapping, selection and copy stay as they are (copying yields the Unicode text). Keep the system emoji font as the fallback for anything the set lacks. Check crates.io before adding a rasteriser.

**Blocked by:** none (v1 18 media pipeline and v1 26 inline custom emoji exist)

**Status:** resolved

**Mockups:** `docs/design/mockups/expressions-in-messages.html`, `docs/design/mockups/reactions.html`, `docs/design/mockups/emoji-picker.html`, `docs/design/mockups/midnight-discord.html`

**Spec:** `docs/design/midnight.md`, Unicode emoji

- [x] Every emoji in the gemoji table resolves to a Twemoji image, sequences included
- [x] Text with emoji wraps, selects and copies as before
- [x] The bundle size is measured and stated in the answer; attribution is in `THIRD_PARTY_NOTICES.md`
- [x] No network is used to fetch emoji

## Answer

Merged to main as `ab147ba..6872c25`. Docs: new `docs/emoji.md` (including every place still drawn in the system font), `docs/ui.md`, `docs/architecture.md`, `README.md`, `THIRD_PARTY_NOTICES.md`.

- Set: crate `crates/twemoji` (package `kyoko-twemoji`, no UI dependency, only zstd, already in the tree via reqwest) with `assets/twemoji.pack` (marked binary in `.gitattributes`): all 4,009 SVGs of jdecked/twemoji v17.0.3 unchanged, in ~512 KiB zstd-19 blocks plus an index, compiled into the binary. `cargo xtask twemoji` rebuilds it from `https://github.com/jdecked/twemoji/archive/refs/tags/v17.0.3.tar.gz` (or `--archive <file>`), refusing a tarball whose SHA-256 isn't `a0855654b633045ae2337537e77f1bb4361162f7fcd910e613eaab1d6d9c5fca`; output is byte-identical every run.
- Resolution: files keyed by code points without U+FE0F, so both spellings of skin tones, ZWJ, flags (incl. England/Scotland/Wales) and keycaps find the same file. `split` takes the longest sequence at each point, falls back to parts, leaves anything without a file to the system font; ©®™ only with U+FE0F; U+FE0E keeps text. ASCII fast path (6.7 KB splits in 32 µs). `bridge::toned` now tones every person in hand-holding sequences.
- Drawing: no new rasteriser; zgx's own SVG nodes (resvg 0.46 inside zgx) rasterise at window scale with zgx's bounded cache, crisp at 150%. `apps/desktop/src/emoji.rs` loads each `SvgData` once. Messages and embeds inline 22 and jumbo 48 through the same placeholder-and-overlay path as custom emoji; reactions 16; picker cells, preview, rail and tone button; custom status button and profile status lines; onboarding options; Luau composer suggestions and the user panel status emoji (Rust sends SVG source, Luau draws `ui.svg { code = … }`). Nothing in a closed spoiler is drawn over.
- Copy fixed: the placeholder's own text is the copy text (the emoji, or `<:name:id>` / `<a:name:id>`), drawn at 1 pt, negative letter-spacing, transparent, padded to the emoji's size; zgx's Ctrl+C and Copy give the right plain text and HTML with no zgx change. Wrapping unchanged.
- Sizes and memory: raw SVGs 10.1 MB; pack 1,298,833 bytes; binary +1,298,432 bytes; index ~0.76 MB built on first use in ~5 ms; one cached 512 KiB block; ~2.5 KiB SVG per emoji shown; raster 4.4 KB inline, 20.7 KB jumbo, 9.2 KB per picker cell at 150% inside zgx's 32 MiB / 1,024-entry SVG cache; decompressing all files 17 ms.
- Attribution: CC-BY 4.0 for the graphics and MIT for the code, with release, URL and checksum, in `THIRD_PARTY_NOTICES.md`, plus a new Credits section in Settings → Advanced. The emoji table itself comes from GitHub's gemoji `db/emoji.json` (MIT), already attributed (v1 story 6).
- Tests: `every_emoji_in_the_gemoji_table_resolves_with_every_skin_tone` (3,463 sequences; pins the 9 table emoji Twemoji 17.0.3 lacks: cracking_face, the two thumb signs, monarch_butterfly, pickle, lighthouse, meteor, eraser, net_with_handle, which fall back to the system font; cracking_face shows a box), sequence tests, `copying_a_message_yields_its_unicode_emoji_and_custom_emoji_markup`, `unicode_emoji_never_reach_the_network` (mockcord recording: only custom emoji and the avatar fetched). Checks re-run by the orchestrator in lane B.
- Parity: oxidcord timeline screens 4.7–7.5% (at or below 26's numbers), emoji-picker 7.5%, user-panel 6.2%, custom-status 4.1%, settings-advanced 4.5%; mockup timeline 19.0%, emoji-picker 31.6% (layout is a later ticket; the emoji art matches the mockups).
- Still in the system font (listed in `docs/emoji.md`): names (servers, channels, threads, members, DMs, authors), status and activity lines, plain-text snippets (reply previews, search, pins, forum), embed titles and field names, polls (answer emoji not drawn at all), forum tags, the tone menu and picker footer label, the composer text field (a zgx textarea holds text only, so in-field emoji need a rich editor). Luau `ui.rich` spans can take an SVG icon; single-line truncation with inline icons is unchecked.
- For M08 (picker): `CELL`/`CELL_IMAGE` in `pickers/emoji.rs` to 48/40; footer and search placeholder can use `emoji::glyph(text, 28)`. M06 (reactions): chips use `emoji::svg_of(name)` at `REACTION_EMOJI`; reuse `emoji::glyph`. M09 (autocomplete): suggestions carry `svg` for Luau. M05 (timeline, popouts): `emoji::whole(text)` gives one emoji's file; new inline kinds reuse `placed(…, copy_text, art, style)` via `body::text` → `inline::placeholder`. Elsewhere: Rust `emoji::glyph(text, side)`; Luau `crate::emoji::luau_code(text)` drawn with `ui.svg { code = … }`.