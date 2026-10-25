# Bounded caches

`state` keeps memory bounded however long kyoko runs. Every limit lives in one struct, `state::CacheLimits`, passed through `SessionConfig::caches` (re-exported as `session::CacheLimits`). The reducer enforces the limits after every event. The UI never trims anything itself.

| Limit | Default | Why |
|---|---|---|
| `messages_per_channel` | 150 | The resting window of a channel: three 50-message pages, which is a full screen plus two pages of scrollback and room for live traffic before anything is dropped. Keep it at or above the 50-message page size. |
| `history_ceiling` | 1000 | The most a window may widen to while the user reads back: 20 pages. |
| `warm_channels` | 8 | Channels that keep message bodies: the open one plus the last seven visited, enough for quick switching. |
| `members` | 5000 | Guild members across all guilds. |
| `users` | 10,000 | Users. Kept above `members`, because a user with a cached member entry is never evicted. |
| `profiles` | 128 | Fetched user profiles (popouts). They are refetched on demand. |
| `referenced_messages` | 256 | Originals of replies fetched by the backfill (`docs/rest.md`), and the ones known to be unavailable. |

The sweeps evict down to 7/8 of a limit, so they run rarely. Eviction removes data from memory only. The sqlite message cache, and anything ticket 16 keeps there, is left alone.

## Warm and cold channels

`State::warm_channels()` lists the warm channels, least recent first. A channel is warmed when it is selected, when history loads for it, and when a message is sent to it. When more than `warm_channels` are warm, the least recent one goes cold and its `ChannelMessages` are dropped. The open channel and a channel with pending sends never go cold.

A cold channel keeps only its metadata: the channel itself with its `last_message_id`, its read state and its mention count. Unread and mention badges come from those, so they stay correct, and a live message in a cold channel still updates them. Opening a cold channel loads it again, from the sqlite cache first and then from REST.

## The message window

Each `ChannelMessages` holds a contiguous window of messages. `window(limits)` is its current size limit, `resident()` is the range of ids it holds, and `is_resident(first, last)` says whether a range is in memory.

- Normally the window is `messages_per_channel` and the oldest messages are dropped as live messages arrive.
- An older page loaded into the open channel widens the window up to `history_ceiling`. Past the ceiling the newest messages are dropped instead. The window becomes detached and `LoadNewer` fetches them again.
- Newer pages widen the window the same way and drop the oldest messages.
- The window shrinks back to `messages_per_channel`, dropping the oldest messages, when the user returns to the bottom (a viewport with `at_bottom` on a window that is not detached) or leaves the channel.

## Referenced messages

Originals fetched for replies live in their own least-recently-used store, never in a channel window, so they cannot widen a window or push out a visible row. `State::reply_target(message)` resolves a reply in order: the `referenced_message` Discord embedded, the channel window, the store. It returns `ReplyTarget::Message`, `Unavailable` (deleted or not readable) or `Loading`. A channel going cold drops its entries. A MESSAGE_DELETE of a stored original turns it unavailable, and a MESSAGE_UPDATE edits it.

## The viewport

The UI reports what it shows with `Command::SetViewport { channel_id, viewport: Viewport { first, last, at_bottom } }`. A viewport change publishes no `Changes` unless the window shrinks. Trimming never drops a message between `first` and `last`. If the visible range blocks the preferred end, the window takes from the other end, and it widens up to the ceiling when live messages arrive while the user reads back. Leaving the channel clears its viewport. The timeline (ticket 25) keys rows by message id and anchors on them, so it should send the viewport whenever its first or last visible row changes and when it reaches the bottom.

## People

Members, users and profiles are least-recently-used caches (`state::Recent`). An entry counts as used when it is written or seen again in an event. When a cache is over its limit, the least recently used entries go first, except:

- the current user, in `users` and in every guild's members, so mention checks keep working;
- authors and mentioned users of messages in warm channels, and users typing, for the guilds those channels belong to;
- the open guild's member-list rows and every guild's voice-state users;
- for `users` only, relationships, DM recipients and every user that still has a member entry;
- for `profiles`, the current user.

An evicted user's presence is dropped too. A member list for a guild that isn't open can then show rows without role colours until the guild is opened again and Discord resends the list.
