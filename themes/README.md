# Themes

First party themes. A theme is one `theme.json` of token overrides on top of a built-in theme, with no code. The format and the token list are in `docs/themes.md`. These themes ship with Kyoko and are listed under Built In in Settings, Themes, where Apply makes one the base theme.

| Theme | What it looks like |
| --- | --- |
| `midnight` | Deep blue dark theme with the violet accent |
| `amoled-violet` | True black, violet accent, rounder corners |

## Layout

```
themes/<id>/theme.json
themes/<id>/theme.toml
themes/<id>/README.md
```

`theme.toml` holds `id`, `name`, `version`, `author` and `description`. The `id` matches the folder name and `[a-z0-9-]{1,64}`.

## Add a theme

1. Make a folder `themes/<id>/` with `theme.json` and `theme.toml`.
2. Run `cargo xtask theme check`. It checks every folder: the `theme.toml` fields, the id, that `theme.json` is a JSON object under 512 KB and that its `base` is `dark`, `light` or `amoled`.
3. Open a pull request. Bump `version` in `theme.toml` for every later change to `theme.json`.

There is no catalog and no signing key. Themes carry no code, so anyone can share one as a file or a link, and users add it in Settings, Themes (`docs/themes.md`).
