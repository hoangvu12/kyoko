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

`apps/desktop/ui/icons/` holds the icon set oxidcord 0.2.0-beta.1 uses, taken from the `gpui-kit-assets` 0.8.0 crate that bundles them. Each file is named after oxidcord's `IconKind` in snake case, except `circle_x`, `circle_check` and `info`, the icons gpui-component's in-app notifications draw, which oxidcord's toasts use. 121 of the 125 files are [Lucide](https://lucide.dev) 1.43.0 icons, unchanged. The SilentTyping plugin's `plugins/silent-typing/keyboard.svg` and `keyboard_off.svg` are Lucide's `keyboard` and `keyboard-off`, and the ExpressionCloner plugin's `plugins/expression-cloner/copy_plus.svg` is Lucide's `copy-plus`, unchanged. Lucide is distributed under the ISC licence, and the icons Lucide derived from Feather under the MIT licence:

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

