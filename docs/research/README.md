# zgui Discord client: research index

Research for a lightweight native Discord client with a Rust core and a zgui/Luau UI. Read the files in order.

zgui has since been renamed zgx (`zeronsh/zgx`); the notes below keep the old name because they cite zgui@a9dc282 sources.

1. `core.md` — the starting question: can we build it on zgui? It covers zgui's runtime capabilities (Part A) and how a Discord user-account client must talk to Discord (Part B), with concord as the reference implementation.
2. `oxidcord-review.md` — an assessment of whether to fork `Master0fFate/oxidcord` (MIT/Apache) instead of writing our own core. Verdict: fork only the crates we need and own that copy (option c). It includes a grade for each crate, its defects, and an estimate for migrating the UI to zgui.
3. `vencord.md` — whether we can support Vencord. Real Vencord can't run in a native client; we should build a plugin system modelled on it. Includes a triage of its 168 stock plugins, a proposed plugin-system design, and the first plugins to ship.
