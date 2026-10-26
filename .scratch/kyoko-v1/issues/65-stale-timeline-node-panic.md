# 65 — Stale timeline node panic when the slot collapses

**What to build:** Fix the intermittent `zgx: contained a panic (stale scene node)` seen while Midnight 21 captured Home screens (about 3 in 20 runs, every friends screen). The backtrace is in `kyoko::timeline::inline::rich_node` (`apps/desktop/src/timeline/inline.rs:134`), called from `inline::place` after layout: it reads a timeline row node that was already removed when Home collapsed the timeline slot. zgx contains the panic but shows its error banner. Inline placement (custom and Unicode emoji laid over their placeholder rects, 26/M04) must skip or drop rows whose nodes are gone instead of reading them.

**Blocked by:** Midnight 21

**Status:** ready-for-agent

- [ ] A test reproduces the panic deterministically (collapse the slot, or remove rows, between layout and the inline placement pass) and passes after the fix
- [ ] 20 consecutive Home/friends captures show no zgx error banner
- [ ] No inline emoji go missing or misplaced in the timeline captures
