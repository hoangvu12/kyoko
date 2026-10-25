# 57 — Themes from files and links, and colour import

**What to build:** Users can switch themes, add themes from local files or from links (like Vencord's Local Themes and Online Themes), and import the colours of a Vencord or BetterDiscord theme.

**Blocked by:** 37 (Account and settings pages)

**Status:** claimed

Decided with the user (2026-10-26): no signing key and no catalog server. Themes carry no code, so HTTPS from the link plus strict validation is enough. The signed-catalog leftovers from oxidcord (`session/src/catalog.rs` fail-closed model, `xtask theme keygen`/`sign`, `themes/catalog.json` instructions in `themes/README.md`) are removed or rewritten to match.

How Vencord does it (behaviour reference only, GPL, don't copy code): the Themes tab has Local Themes and Online Themes. Online themes are a text box of links, one per line, stored as a list (`themeLinks`); blanks and duplicates are dropped; a line may start with `@light` or `@dark` to apply only in that mode; links must be direct links to the file (raw or github.io). Local themes are `.css` files in a themes folder, each toggled on or off (`enabledThemes`, by file name), with Open Themes Folder and a refresh button. Theme metadata comes from the CSS file's first `/** … */` comment (`@name`, `@author`, `@description`, `@version`, `@source`, `@website`), the format BetterDiscord uses; the name defaults to the file name.

- [ ] A Themes page (in 45's "Kyoko" settings section) matches oxidcord's Themes page in layout and lists the bundled themes, local themes and online themes, each showing name, author, description and version
- [ ] Local themes: files in `<data>/themes/` (a kyoko `theme.json`, or a `.css` file), toggled on or off, Open Themes Folder, and they reload when the folder changes
- [ ] Online themes: a links box like Vencord's (one per line, blanks and duplicates dropped, optional `@light`/`@dark` prefix); links are fetched by the session, never the UI, over HTTPS only, with no Discord token, cookies or super properties, a size limit and a timeout; the last good copy is cached on disk and used offline; a failing link shows its error and never breaks the others
- [ ] Importing a Vencord or BetterDiscord CSS theme maps its colour variables to kyoko's tokens and ignores everything else; unknown or invalid values are skipped, never fatal
- [ ] Enabled themes apply live, in order, on top of the chosen base theme (dark, light, amoled, system); turning one off restores the colours underneath
- [ ] Session or bridge tests cover link fetching (HTTPS only, size limit, timeout, cache fallback, no auth headers), CSS colour import and metadata parsing, and the apply order
