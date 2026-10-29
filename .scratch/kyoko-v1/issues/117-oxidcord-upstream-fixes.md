# 117 — Port three oxidcord upstream fixes

**What to build:** kyoko forked oxidcord at `b852253`; upstream has 41 commits since. A trial cherry-pick of 18 candidates applied none cleanly, so useful fixes are hand-ported (spec "Upstream changes are never merged wholesale"). The user approved (2026-10-29) porting these three now and a light routine for later reviews. The main checkout has an `upstream` remote (`https://github.com/Master0fFate/oxidcord.git`, `--no-tags`); read commits with `git show <sha>` from any worktree, or from `oxidcord-ref` (source only, never build or run oxidcord).

**Status:** claimed (lane A)

- [ ] **1066772** "Fix permissions and hide channels you cannot open": READY members that carry only `user_id` (user objects deduped) are dropped, so our own roles never load and permissions can be wrong. kyoko's `ready.rs` has the same bug: fix it, then check send and attach gating (composer and session) uses the now-correct permissions. Port the channel-hiding parts only where kyoko doesn't already match Discord; skip the See Hidden Channels plugin, the catalog and the oxidcord UI/locales (kyoko's UI is Luau)
- [ ] **c6ea319**: only the `PIN_MESSAGES` and `BYPASS_SLOWMODE` permission bits and `can_pin` (pin actions gated on Pin Messages, not Manage Messages). Skip the message-menu rebuild and speech
- [ ] **c494901**: secret storage only: owner-only ACL on private files on Windows, no plaintext fallback when the keyring fails (fail visibly instead), atomic writes keep the ACL. Check whether the "links only open http/https" and "oversized video frames dropped" parts apply to kyoko (kyoko's link opener and `crates/video`); port them if they do, say so if not. Skip the plugin sandbox network removal unless kyoko's plugin host grants the same access (check and report)
- [ ] Behaviour tests at the session seam for each fix (READY with deduped members gives the right own permissions; pin gating; keyring failure gives no plaintext file; ACL set on Windows)
- [ ] One commit per upstream fix, body ending `Ported from oxidcord <full sha>`
- [ ] Report, per commit, what was ported, what was skipped and why

## Routine for later upstream reviews

- The orchestrator keeps the last reviewed upstream commit in `.scratch/upstream/last-reviewed` (`b852253` until this ticket lands). Review with `git log --oneline <last-reviewed>..upstream/main`.
- Small core fixes: port with a path-limited `git diff X^ X -- crates/... | git apply -3`, adapt, test, commit "Ported from oxidcord <sha>".
- UI features become tickets naming the upstream sha. Candidates already seen: d0b2495 mention chime / DND, fc1dc43 provider embeds, 88dced0 + db5bfa6 GIF performance, b3c036b paste/drop files.
