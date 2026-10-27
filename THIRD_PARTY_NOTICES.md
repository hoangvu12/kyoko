# Third-party notices

## gemoji

`crates/protocol/src/expressions/unicode_table.rs` is derived from `db/emoji.json` in [github/gemoji](https://github.com/github/gemoji). All 1,870 gemoji entries appear in the table with their emoji, aliases and tags, the tags used as search keywords. The table adds Discord-style `flag_xx` shortcodes and two-letter aliases for flags, and 53 newer emoji that are not in gemoji, named after their Unicode names and carrying no keywords.

gemoji is distributed under the following licence:

```
Copyright (c) 2019 GitHub, Inc.

Permission is hereby granted, free of charge, to any person
obtaining a copy of this software and associated documentation
files (the "Software"), to deal in the Software without
restriction, including without limitation the rights to use,
copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
OTHER DEALINGS IN THE SOFTWARE.
```

## Twemoji

Kyoko draws Unicode emoji with [Twemoji](https://github.com/jdecked/twemoji), maintained by Jason Sofonia and Justine De Caires after Twitter. `crates/twemoji/assets/twemoji.pack` holds the 4,009 SVG files of `assets/svg/` in the jdecked/twemoji v17.0.3 release (`https://github.com/jdecked/twemoji/archive/refs/tags/v17.0.3.tar.gz`, SHA-256 `a0855654b633045ae2337537e77f1bb4361162f7fcd910e613eaab1d6d9c5fca`), unchanged, compressed with zstd by `cargo xtask twemoji`. Nothing is fetched at run time.

Copyright 2014–2021 Twitter, 2022–present Jason Sofonia and Justine De Caires, and other contributors. The graphics are licensed under the Creative Commons Attribution 4.0 International licence (CC-BY 4.0), <https://creativecommons.org/licenses/by/4.0/>. Kyoko's own code that finds the file for an emoji sequence follows Twemoji's file naming and copies no Twemoji code; Twemoji's code is distributed under the following licence:

```
MIT License

Copyright (c) 2022–present Jason Sofonia & Justine De Caires
Copyright (c) 2014–2021 Twitter

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Settings → Advanced → Credits names Twemoji and its licence in the app.

## Lucide icons

`apps/desktop/ui/icons/` holds the icon set oxidcord 0.2.0-beta.1 uses, taken from the `gpui-kit-assets` 0.8.0 crate that bundles them. Each file is named after oxidcord's `IconKind` in snake case, except `circle_x`, `circle_check` and `info`, the icons gpui-component's in-app notifications draw, which oxidcord's toasts use. 121 of those 125 files are [Lucide](https://lucide.dev) 1.43.0 icons, unchanged. `kyoko_mark.svg`, `refresh.svg`, `circle_alert.svg`, `key.svg`, `globe.svg`, `warning_fill.svg` and `shield_alert.svg` beside them are kyoko's own, drawn for the Midnight mockups. So are the expression picker's seventeen `picker_*.svg` icons (trophy, stars, clock, the eight emoji categories, trending, back, lock, chevron and search), taken from the M08 mockups' own drawings, and `sparkle.svg`, `add_reaction.svg` and `add_super_reaction.svg`, taken from the M06 reaction mockups. `play_fill.svg`, `pause_fill.svg`, `volume_fill.svg`, `volume_mute_fill.svg` and `file_audio.svg` are kyoko's own too, taken from the M18 voice message mockup's drawings. `poll.svg` is kyoko's own, taken from the M17 poll mockups' drawing, and so is `apps.svg`, taken from the M15 apps launcher mockup's drawing. `star_filled.svg` is Lucide's `star` filled with the stroke colour, and `mark_read.svg` is Lucide's `message-circle` with Lucide's `check` stroke inside it, both added for the timeline. `chat_plus.svg`, `user_plus.svg` and `panel_right.svg` are Lucide's `message-circle-plus`, `user-plus` and `panel-right`, added for Home and DMs. The SilentTyping plugin's `plugins/silent-typing/keyboard.svg` and `keyboard_off.svg` are Lucide's `keyboard` and `keyboard-off`, the ExpressionCloner plugin's `plugins/expression-cloner/copy_plus.svg` is Lucide's `copy-plus`, the ReadAllNotificationsButton plugin's `plugins/read-all-notifications/check_check.svg` is Lucide's `check-check`, and the PlatformIndicators plugin's `plugins/platform-indicators/monitor.svg`, `smartphone.svg` and `globe.svg` are Lucide's `monitor`, `smartphone` and `globe`, unchanged. The GameActivityToggle plugin's `plugins/game-activity-toggle/gamepad.svg` is Lucide's `gamepad`, unchanged, and `gamepad_off.svg` is that icon with its outline opened around Lucide's diagonal slash, kyoko's own variant. Lucide is distributed under the ISC licence, and the icons Lucide derived from Feather under the MIT licence:

```
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

---

The following Lucide icons are derived from the Feather project:

airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, check-circle, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out

The MIT License (MIT) (for the icons listed above)

Copyright (c) 2013-present Cole Bemis

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## GPUI Kit window icons

`apps/desktop/ui/icons/window_minimize.svg`, `window_maximize.svg`, `window_restore.svg` and `window_close.svg` are GPUI Kit's own window-control icons from the `gpui-kit-assets` 0.8.0 crate ([longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)), unchanged. They are distributed under the Apache License, Version 2.0, whose full text is in `LICENSE-APACHE`.

## Audio decoding

Kyoko plays voice messages and audio attachments with crates.io packages linked unmodified: Symphonia 0.6 (`symphonia` and its format and codec crates, Mozilla Public License 2.0, source at [pdeljanov/Symphonia](https://github.com/pdeljanov/Symphonia)), `opus-decoder` 0.1 (MIT OR Apache-2.0), `ogg` 0.9 (BSD-3-Clause) and `cpal` 0.18 (Apache-2.0). The MPL-2.0 files are used as published; their source is available from the link above.

The audio fixtures in `tools/mockcord/fixtures/audio/` (`voice-message.ogg`, `voice-reply.ogg`, `rainy-loop.mp3`) are tones generated for kyoko with FFmpeg's `sine` source; they are test data and not shipped in the application.

## TLS and HTTP/2: BoringSSL and wreq

Kyoko's REST and gateway connections run on [wreq](https://github.com/0x676e67/wreq) and link a patched [BoringSSL](https://boringssl.googlesource.com/boringssl) built from source by `btls-sys`. They are used unmodified, from git pins in the workspace `Cargo.toml` (`docs/fingerprint.md`, "Pins"):

- BoringSSL (vendored in `btls-sys` at rev `129887582a538b8f4dcf371d15c953335312ca37`, with btls's patches): Apache License 2.0. Its fiat-crypto code (`third_party/fiat`) is used under Apache-2.0, one of its MIT, Apache-2.0 and BSD-1-Clause options. BoringSSL's Go-licensed TLS test suite is not compiled into kyoko.
- `wreq` 0.16.1, `wreq-util` 0.2.0, `wreq-proto` 0.2.6, `wreq-rt` 0.2.2-rc.4 and `btls` 0.5.6 (0x676e67 and contributors): Apache License 2.0, whose full text is in `LICENSE-APACHE`.
- `tokio-btls` 0.5.6: MIT OR Apache-2.0, used under Apache-2.0.
- `btls-sys` 0.5.6 and `http2` 0.5.20 (a fork of `h2`): MIT, licence below.
- `chromium-roots` 0.1.2, Chromium's root store as Rust data: BSD-3-Clause, licence below, for both the crate and the Chromium data it is generated from.

`btls-sys` and `http2` are distributed under the following licence:

```
Copyright (c) 2014 Alex Crichton
Copyright (c) 2020 Ivan Nikulin <ifaaan@gmail.com>
Copyright (c) 2025 0x676e67 <gngppz@gmail.com>
(btls-sys)

Copyright (c) 2017 h2 authors
(http2)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

`chromium-roots` and the Chromium root data are distributed under the following licence:

```
Copyright (c) 2026, chromium-roots contributors
Copyright 2015 The Chromium Authors
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Fonts

Kyoko bundles one font family, Inter, for the built-in Midnight theme. `apps/desktop/fonts/inter/` holds `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf` and `Inter-Bold.ttf` (weights 400, 500, 600 and 700) from `extras/ttf/` of the [rsms/inter](https://github.com/rsms/inter) release v4.1 (`Inter-4.1.zip`, SHA-256 `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e`), unchanged, with the release's `LICENSE.txt`. Their SHA-256 sums:

```
40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82  Inter-Regular.ttf
97ad806f526e41546d46365bb3a393145f75b7b1568913db74549ad8b8dba872  Inter-Medium.ttf
78a843fade9d4612a5567302fb595b56976eb5fcebf4fea5a5912d638bafcde3  Inter-SemiBold.ttf
288316099b1e0a47a4716d159098005eef7c0066921f34e3200393dbdb01947f  Inter-Bold.ttf
```

Inter is Copyright (c) 2016 The Inter Project Authors and is distributed under the SIL Open Font License, Version 1.1, whose full text is in `apps/desktop/fonts/inter/LICENSE.txt`. The fonts are compiled into the executable and registered with the window at start; nothing is downloaded.

Every other family comes from the operating system. The Midnight theme asks for `Inter`, `gg sans`, `Noto Sans`, `Helvetica Neue`, `Helvetica`, `Arial` and `sans-serif` for the interface and `gg mono`, `Source Code Pro`, `Consolas`, `Monaco` and `monospace` for code; the Dark, Light and AMOLED themes ask, like oxidcord, for `gg sans`, `Noto Sans`, `Segoe UI`, `Helvetica Neue`, `Arial` and `sans-serif`, and `Consolas`, `Menlo` and `monospace`. Discord's gg sans and gg mono are proprietary and are used only if the user has installed them.

## Oxidcord theme and layout

The theme tokens in `crates/bridge/src/themes/tokens.rs` and `apps/desktop/src/theme.rs` (colours, sizes, fonts, the light and AMOLED overrides) and the shell layout in `apps/desktop/ui/shell/` are ported from oxidcord's `crates/ui/src/theme.rs` and `crates/ui/src/shell/`, under oxidcord's MIT OR Apache-2.0 licence (see `NOTICE`).

## Midnight theme

The built-in Midnight theme is refact0r's Midnight, Vencord flavour 2.1.1 ([refact0r/midnight-discord](https://github.com/refact0r/midnight-discord)). Its palette values (`MIDNIGHT` in `crates/bridge/src/themes/tokens.rs`), its variable names, which kyoko's Midnight tokens and the CSS import follow, and the panel layout described in `docs/design/midnight.md` come from it. `crates/bridge/tests/bridge/fixtures/midnight-vencord.theme.css` is the flavour file itself (`themes/flavors/midnight-vencord.theme.css` at commit `8598b2d`), unchanged, used only by tests and not shipped in the application. No selectors or rules of Midnight's stylesheet are used. Midnight is distributed under the MIT License:

```
MIT License

Copyright (c) 2025 refact0r

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

`themes/deep-blue/` is a different theme, by Master0fFate, that was named Midnight before this one.

## ClearURLs rules (not bundled)

The bundled ClearURLs plugin (`plugins/clear-urls/`) does not include the ClearURLs project's rule list ([ClearURLs/Rules](https://github.com/ClearURLs/Rules)), which is distributed under the GNU Lesser General Public License v3.0. Its rule set, `plugins/clear-urls/rules.luau`, is kyoko's own, written from the publicly documented names of tracking parameters (for example Google Analytics' `utm_*` campaign parameters and click identifiers such as `gclid` and `fbclid`), and is distributed under kyoko's licence. No rules are downloaded at run time.

