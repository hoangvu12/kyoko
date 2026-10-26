# 22 — Threads, forums, inbox, pins, search and the quick switcher

**What to build:** Restyle the thread list, split view and creation, the forum channel, post and new post, the inbox, pins, search results and the quick switcher to the mockups.

**Blocked by:** 03, 07; v1 31, 33

**Status:** resolved

**Mockups:** `docs/design/mockups/threads-list.html`, `docs/design/mockups/threads-split-view.html`, `docs/design/mockups/threads-create.html`, `docs/design/mockups/forum.html`, `docs/design/mockups/forum-post.html`, `docs/design/mockups/forum-new-post.html`, `docs/design/mockups/inbox.html`, `docs/design/mockups/pins.html`, `docs/design/mockups/search-results.html`, `docs/design/mockups/quick-switcher.html`

**Spec:** `docs/design/midnight.md`, Structure: floating panels, Other surfaces

- [x] Matches the mockups (all but the inbox, which kyoko does not have; see the Answer)

## Answer

Merged to main as `6ffdd9e..6a3fdf6`. Docs: `docs/ui.md`, `docs/parity.md`.

- Threads popout (`src/threads/popouts.rs`, Rust above the slot): Discord's sections replace Active/Archived tabs ("N Joined Threads", "N Other Active Threads", "Older Threads"; archived threads load when it opens), a search-by-name field, cards with name, avatar, author and latest snippet, time and message count; placed under the channel header (fixed offsets from the press point, not the button's bounds). Join/Leave moved off the cards to the thread panel header.
- Pins popout: message cards with a 40-point avatar, Jump and Unpin on hover.
- Thread panel (`thread_panel.luau`): Midnight header and notice; forum posts get a wider panel with tags. **Thread creation is inline in the side panel** (name, starter message, Private Thread switch, "Enter a message to start the conversation!" field; the typed message is sent into the new thread once it opens). `CreateThread` gained `private` (type 12, gated on Create Private Threads; only for new threads in a text channel, never from a message, as Discord). "Hide after inactivity" is gone (24-hour default). The Create Thread and Create Post dialogs (`threads/dialogs.rs`) are deleted.
- Forum (`forum.luau`): "Search or create a post..." (filters by title, Enter starts a post with that title), New Post as an inline form at the top, Sort & View menu (sort, list/gallery), tag chips with Clear, Midnight cards; the member list is hidden while a forum is open.
- Search panel: 420 wide, Messages/Media/Links/Files/Pins tabs (re-run with `has:image`, `has:link`, `has:file`, `pinned:true`), results grouped by channel with highlighted words and Jump on hover, Newest/Oldest as a real sort (`oldest_first`, mockcord honours it), pages of 25 instead of infinite scroll.
- Quick switcher restyled; its capture types "gen".
- Tests: session `a_private_thread_is_created_as_one_and_opens`, `an_oldest_first_search_asks_for_ascending_order_and_gets_it`; bridge thread-list test rewritten for the sections; load check gains "a new thread" and "a new forum post".
- Checks re-run by the orchestrator in lane C: all green. Mockup parity (port 8493, before → after): threads-list 14.6 → 14.1%, thread-panel 13.7 → 13.7%, thread-create 86.7 → 11.7%, forum 12.4 → 9.0%, forum-post 15.8 → 15.0%, forum-post-create 74.1 → 9.3%, pins 14.5 → 13.8%, search 14.5 → 14.9%, quick-switcher 13.8 → 18.1% (mock data has only three "gen" channels, so the list is shorter than the mockup's; layout matches).
- **Inbox: not built.** Kyoko has no inbox (the header button has no surface, no recent-mentions data); `inbox.html` stays unmapped. Building it needs the recent-mentions REST call, session state, a mockcord route and a Rust popout layer (asked the user whether to add a ticket).
- Left out: forum cards' reaction chip, thumbnail and "1 new"; dashed "Add Tags" border (zgx has none); the thread panel header keeps archive, join/leave and close instead of bell, pin, more; no "N messages" divider in a post; Media searches `has:image` only; the header search field doesn't show the running query with a clear X.
- For later tickets: `profile::parts` is `pub(crate)` (`parts::size` reads token numbers in Rust). Luau: `ui.each` needs a state, so wrap lists in `ui.computed`; passing a shared row straight into a `ui.component` fails with "attempt to modify a readonly table", wrap it as `{ row = row }`.
