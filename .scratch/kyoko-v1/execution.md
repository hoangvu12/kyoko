# kyoko v1: execution plan

How to implement the tickets in `issues/` as fast as possible without overloading the development PC.

**Target machine:**
- i5-14400F (10 cores / 16 threads)
- 32 GB RAM (about 13 GB typically free alongside other work)
- about 116 GB free on C:
- RTX 5050
- sccache installed

## What limits speed

The dependency graph sets the floor, not the machine. The longest chain is about ten tickets deep:

01 → 02 → 03 → 04 → 19 → 20 → 25 → 26 → 28 → 53

No amount of parallelism shortens it. The graph is only wide in two places:

- **Core:** 06–18, about 12 tickets, once 05 lands.
- **UI parity:** 21–40, about 18 tickets, once 20 lands.

So the plan keeps the chain moving with one dedicated lane, and uses spare capacity on the wide parts.

## Lanes

**At most 3 implementation lanes run at once.** More lanes would mostly wait on the chain while competing for RAM, disk and CPU.

| Phase | Lanes | Tickets |
|---|---|---|
| Spike | 2 | Lane A: 01 → 02 → 03 → 04 (strictly in order). Lane B: 05 once 02 lands |
| Core and UI foundation | 3 | Lane A: 18, 19 → 20 → 25 → 26 → 28 (the critical chain). Lanes B and C: the core tickets 06–17 |
| UI parity | 3 | Lane A keeps the chain. Lanes B and C take unblocked screens 21–40 |
| Logins, plugins, release | 3 | 41–58 as they unblock, plugins after 45 |

**How lanes pick work:**
- Take the frontier: unblocked, unclaimed tickets, lowest number first.
- Lane A always prefers chain tickets.
- A ticket is claimed before work starts, by setting its Status line.

## Resource rules

1. **One persistent worktree per lane, reused across tickets.**
   - Each lane rebases onto main between tickets instead of creating a fresh worktree. That keeps incremental builds warm and disk use bounded to three target directories.
   - Lanes don't share a target directory: cargo's lock would serialize them, and switching branches would make them rebuild each other's work.
2. **One shared sccache for every lane.**
   - Add the kyoko checkout and each lane worktree to `basedirs` in `%APPDATA%\Mozilla\sccache\config\config`, then restart the server (`sccache --stop-server; sccache --start-server`).
   - Dependency crates (zgx, wgpu, Luau, reqwest and others) then compile once for all lanes.
3. **Cap build parallelism per lane** with `CARGO_BUILD_JOBS=5`, so three lanes together use about the whole CPU without freezing the desktop. Drop to 2 lanes if RAM pressure appears: heavy linking, or free RAM below about 4 GB.
4. **Core tickets build only the headless crates.**
   - Tickets 05–17 build and test the core packages (`-p`), never the zgx app.
   - Those builds skip wgpu and Luau entirely, so they are fast and light.
5. **UI tickets iterate in Luau.**
   - Screens are Luau scripts that hot-reload without recompiling. Rust rebuilds only when a bridge projection or the timeline view changes.
   - Keep the app running against mockcord while a screen is built.
6. **Tests are cheap by design.** Mockcord runs in-process with controlled time, so pacing and backoff tests finish in seconds.
7. **Parity captures run on demand,** never in a loop. They render both apps once per screen.
8. **Try faster linking in the spike.** Test `rust-lld` as the MSVC linker during 03/04. Keep it if the zgx app links reliably and faster.
9. **Disk hygiene.**
   - The three lane target dirs are the main disk cost.
   - Run `cargo clean` on a lane only when switching it to a very different kind of work, or when C: drops below about 40 GB free.
   - The oxidcord parity reference build (ticket 19) lives in its own checkout and is built once.
10. **Real Discord runs one at a time,** on the throwaway account, by hand, never from parallel lanes. Several clients connecting at once on one account would look abnormal to Discord.

## Merge cadence

- Each ticket merges to main as soon as its acceptance criteria pass, with fmt, clippy and the session tests green.
- Lanes rebase before starting their next ticket.
- Tickets are cut by area, so conflicts between lanes should be rare. When two lanes touch the bridge or the session contract, the later one rebases and adjusts.
