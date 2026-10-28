# 109 — Theme links test ignores stray local probes

**What to build:** `theme_links::theme_links_are_fetched_over_https_only_without_any_discord_credentials` (`crates/session/tests/session/theme_links.rs`, the `plain_served.paths().is_empty()` assertion near line 214) fails on this machine about half the time, alone or under load. A temporary diagnostic in ticket 84 showed why: the plain-http listener receives `GET /` with only `Host: localhost:<port>` and `Connection: close` from some other process that probes newly opened localhost ports (probably the user's Roboco test runs). Kyoko itself checks the scheme before any request and only ever asks for `/ocean.css`. Make the test prove what it means (kyoko never requests the plain http link) without being broken by requests it didn't cause, e.g. assert that no request for the theme's path (`/ocean.css`) reached the plain server, and apply the same rule to any other assertion in the file that counts every request a test listener receives.

**Blocked by:** none

**Status:** claimed

- [ ] The test passes 20 runs in a row alone and 10 in a row during a full workspace build, on this machine
- [ ] A deliberately broken scheme check (plain http fetched) still fails it
- [ ] Any other session test that asserts "no request at all" on a freshly opened localhost listener gets the same treatment (grep for listeners asserting empty request lists)
