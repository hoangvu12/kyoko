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

## Lucide icons

`apps/desktop/ui/icons/` holds the icon set oxidcord 0.2.0-beta.1 uses, taken from the `gpui-kit-assets` 0.8.0 crate that bundles them. Each file is named after oxidcord's `IconKind` in snake case. 118 of the 122 files are [Lucide](https://lucide.dev) 1.43.0 icons, unchanged. Lucide is distributed under the ISC licence, and the icons Lucide derived from Feather under the MIT licence:

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

airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out

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

Kyoko bundles no fonts. Like oxidcord, it asks for the first installed family of `gg sans`, `Noto Sans`, `Segoe UI`, `Helvetica Neue`, `Arial` and `sans-serif` for the interface, and of `Consolas`, `Menlo` and `monospace` for code, all provided by the operating system. Discord's gg sans is proprietary and is used only if the user has installed it.

## Oxidcord theme and layout

The theme tokens in `apps/desktop/src/theme.rs` (colours, sizes, fonts, the light and AMOLED overrides) and the shell layout in `apps/desktop/ui/shell/` are ported from oxidcord's `crates/ui/src/theme.rs` and `crates/ui/src/shell/`, under oxidcord's MIT OR Apache-2.0 licence (see `NOTICE`).

## ClearURLs rules (not bundled)

The bundled ClearURLs plugin (`plugins/clear-urls/`) does not include the ClearURLs project's rule list ([ClearURLs/Rules](https://github.com/ClearURLs/Rules)), which is distributed under the GNU Lesser General Public License v3.0. Its rule set, `plugins/clear-urls/rules.luau`, is kyoko's own, written from the publicly documented names of tracking parameters (for example Google Analytics' `utm_*` campaign parameters and click identifiers such as `gclid` and `fbclid`), and is distributed under kyoko's licence. No rules are downloaded at run time.

