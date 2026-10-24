# 01 — Rebrand fork to kyoko

**What to build:** kyoko stops presenting itself as oxidcord anywhere a user, the OS or a network peer can see it. The fork credits oxidcord and keeps its licences. Contributor rules and CI reflect kyoko's decisions.

**Blocked by:** None — can start immediately

**Status:** claimed

- [ ] Crate packages, binary, app id, data and config directories, env prefixes, window titles and icon all say kyoko. oxidcord's name and icon are no longer used
- [ ] oxidcord's update-signing key, plugin-catalog signing key and GitHub update/catalog endpoints are removed or replaced
- [ ] LICENSE-MIT, LICENSE-APACHE and NOTICE are kept with oxidcord's copyright line. The README has a credit section that links oxidcord and explains what kyoko changes
- [ ] The emoji table's origin is identified and attributed, or the table is replaced
- [ ] Contributor rules require behaviour tests at the session seam, replacing the "no unit tests" rule
- [ ] CI runs on Windows only: build, fmt, and clippy with warnings denied
- [ ] The headless mock run passes as before
