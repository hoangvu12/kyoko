# 65 — Stale timeline node panic when the slot collapses

**What to build:** Fix the intermittent `zgx: contained a panic (stale scene node)` seen while Midnight 21 captured Home screens (about 3 in 20 runs, every friends screen). The backtrace is in `kyoko::timeline::inline::rich_node` (`apps/desktop/src/timeline/inline.rs:134`), called from `inline::place` after layout: it reads a timeline row node that was already removed when Home collapsed the timeline slot. zgx contains the panic but shows its error banner. Inline placement (custom and Unicode emoji laid over their placeholder rects, 26/M04) must skip or drop rows whose nodes are gone instead of reading them.

**Blocked by:** Midnight 21

**Status:** resolved

- [x] A test reproduces the panic deterministically (collapse the slot, or remove rows, between layout and the inline placement pass) and passes after the fix
- [x] 20 consecutive Home/friends captures show no zgx error banner
- [x] No inline emoji go missing or misplaced in the timeline captures

## Answer

Merged to main as `959f3a3`.

- Root cause (kyoko, not zgx): `inline::track` runs its first emoji measurement with `NodeHandle::after_layout`, which zgx documents "runs even if the node has gone by then". When a message row mounts and is removed before the next layout (Home collapsing the timeline slot does this), the queued callback still ran, and `place` → `rich_node` called `scene.kind(owner)` on the removed node, hitting zgx's `assert!(self.contains(id), "stale scene node")`. `on_layout` and `observe_*` were never the problem (zgx skips removed nodes for those).
- Fix: `place` in `apps/desktop/src/timeline/inline.rs` returns `None` when `scene.contains(owner)` is false. Other deferred node reads in `apps/desktop/src/timeline/` were checked and already guarded (`popout::bounds_of`, `TimelineView::newest_gif`, `gif_frames` pruning, `follow_layout`); `moderation/dialogs.rs`'s `after_layout` only focuses. No upstream report needed.
- Tests: `timeline::emoji_tests::a_row_removed_before_its_first_layout_places_no_emoji` panicked with the reported backtrace before the fix and passes after; a row mounted afterwards still shows both Twemoji SVGs. 20 consecutive `parity mockup` runs of the five friends screens (100 captures): no "stale scene node", "contained a panic" or banner. `timeline-inline`, `timeline`, `expressions-in-messages` still place every inline emoji. Checks re-run by the orchestrator in lane B (clippy and the desktop tests again after rebasing over Midnight 06): all green.
- For later tickets: any `after_layout` callback that reads the scene through a handle must check `scene.contains(handle.node())` (or `handle.is_mounted()`) first; zgx enforces it with an assert, not an `Option`.
