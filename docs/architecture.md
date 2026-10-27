# Architecture

Kyoko is a Cargo workspace. Data flows one way: gateway event, state reducer, revision bump, UI re-render of the views whose revision changed. The UI never performs network calls; it sends `Command` values to the session, which owns the REST client and the gateway task.

```
apps/desktop        binary `kyoko`: the zgx host, its Luau interface in `apps/desktop/ui/`, mockcord with `--mock`
crates/protocol     typed Discord REST and gateway models, lenient serde decoding
crates/gateway      websocket session: identify, heartbeat, resume, zlib-stream, send window, backoff
crates/rest         HTTP client: rate-limit buckets, pacing, 401 stop, 403 breaker, captcha errors (docs/rest.md)
crates/state        normalized store, permission math, read state, message cache
crates/store        sqlite cache and settings, token in the OS keyring
crates/markdown     Discord markdown parser to AST
crates/media        image fetching, decoding and the budgeted gallery of host images (docs/media.md)
crates/audio        audio decoding (Ogg Opus, MP3, Vorbis, FLAC, WAV, AAC) and output: the deck, time-stretched speeds, the device, silent and manual sinks (docs/media.md)
crates/twemoji      the bundled Twemoji set: emoji sequences to SVG files, packed with zstd (docs/emoji.md)
crates/session      session core: owns REST, gateway and persistence, takes Commands, emits Changes; fetches theme links (docs/themes.md)
crates/bridge       core to UI bridge without a UI dependency: the Changes inbox, the projections the UI publishes, theme tokens and the theme catalog
crates/login        token, QR and web login
crates/plugins      the Luau plugin runtime: sandboxed VMs on worker threads behind SessionHooks (docs/plugins.md)
crates/platform     Windows tray, notifications, hotkeys, clipboard, autostart, URL scheme (docs/desktop.md)
crates/net          wreq client builder with Chrome's TLS and HTTP/2 options; not used by the app yet (docs/fingerprint.md)
tools/mockcord      mock Discord REST and gateway server driven by fixtures, with traffic recording and scenario controls
tools/xtask         test, headless-run, the first party theme check, release notes, the Twemoji pack, tls-capture
```

Crate graph: `protocol` is the leaf. `rest` and `gateway` depend on `protocol`; `state` on `protocol` and `markdown`; `store` on `protocol`. `session` sits on all of them. `bridge` depends on `session`, `state`, `media` and `audio` (and on `plugins` in its tests only). `audio` depends on no other kyoko crate. `plugins` depends on `session`, `state`, `store`, `protocol` and mlua. `twemoji` depends only on zstd. `net` depends only on wreq and wreq-util, and no other crate depends on it yet. `apps/desktop` depends on `bridge`, `plugins`, `session`, `state`, `store`, `protocol`, `twemoji`, `mockcord` and zgx. None of `protocol`, `rest`, `gateway`, `state`, `store`, `markdown`, `session`, `bridge`, `plugins`, `twemoji` or `net` has a UI dependency.

Behaviour tests drive a session against an in-process mockcord; see `docs/testing.md`. Gateway pacing, reconnect policy and the connection status are in `docs/gateway.md`. The Discord web-client identity (user agent, super-properties, build number, cookies) is described in `docs/fingerprint.md`. Cache limits, warm channels and the message window are in `docs/caches.md`. Risky-action locks and the safety setting are in `docs/locks.md`. Findings from the zgx spike (build times, memory, bridge latency, the real-traffic comparison) are in `docs/spike.md`. The interface (Luau layout, theme, fonts, icons, naming) is described in `docs/ui.md`.

## Session hooks

The session has no plugin runtime; `plugins::PluginHost` implements this interface (`docs/plugins.md`). `SessionConfig::hooks` takes an optional `Arc<dyn SessionHooks>`; every method has a no-op default:

- `before_send(channel_id, content)` and `before_edit(channel_id, message_id, content)` may return a `Rewrite` (the replacement text and, optionally, who rewrote it). A rewritten send or edit is shown as altered by the rewrite's `by`, or `name()` without one; an edit sent unchanged clears the mark.
- `allow_typing(channel_id)` can veto the typing indicator.
- `allow_notification(message)` filters the session's notification alerts, only those the burst limit would show (`docs/desktop.md`). It is also exposed as `Session::should_notify`.
- `rank_emoji(request)` may reorder the emoji autocomplete's candidates for one query; `ranks_emoji()` says whether anything would, so the session skips the debounce and the call when nothing ranks. See "Emoji ranking" in `docs/plugins.md`.
- `on_dispatch(event)` observes every gateway dispatch after it is applied to state, a live READY included. It runs on the session task, so implementations must return quickly and queue any real work.

Text hooks run on a blocking worker. Each call is wrapped in `catch_unwind`; an error or panic is logged with the hook name only, never the message content, and the session continues with the unmodified value.

## Display hooks

`session::DisplayHooks` is the sibling interface for what the window shows rather than what the session does: `format_name(person)` (a `ShownName`: the name and an optional dimmed extra), `format_relative(time)` (the text of a relative time) and `render_message(message)` (a `MessageLook` for a message with kept deleted or edited versions: deleted style, edit history, accessory lines), plus `formatters(kind)`, a list that changes whenever the set of formatting plugins, their settings or their generation changes. The session never calls it; `plugins::PluginHost` implements it, and `bridge::Display` puts a cache in front of it. The UI thread only ever looks into that cache: a miss shows kyoko's own text, queues the question for the `display-hooks` thread, and the answer arrives as a wake (`Formatting::Names`, `Formatting::RelativeTimes` or `Formatting::Messages`) that redraws the rows it changes. See "Display hooks" in `docs/plugins.md`.

## Threads

- The UI thread owns the zgx window, its Luau scripts and every zgx value. zgx values are `Rc` based and never leave it.
- A tokio runtime on background threads owns the gateway socket, REST client and sqlite writer.
- Out of the session: its `notify` callback pushes each `Changes` into `bridge::Inbox`, an unbounded queue with a waker. A task on the UI thread drains it, merges the batch, read-locks `State` once, lets `bridge::Sidebar` and `bridge::Rail` work out what changed and publishes small projections as zgx shared values named `<area>.<name>` (`rail.items`, `sidebar.channels`, `session.status`; the full list is in `docs/ui.md`). zgx's latest-value channel is not used, because it drops intermediate values, and the whole store is never published.
- Into the session: Luau calls host actions (`ui.action("rail.select_guild", id)`), Rust turns them into `Command`s and sends them to the session.
- No lock is held across a frame.

## zgx

zgx (formerly zgui) is a git dependency on `zeronsh/zgx`, pinned to one commit in the workspace `Cargo.toml`, together with the nine crates it patches (`wgpu-hal`, `cosmic-text`, `taffy`, four AccessKit adapters, `winit`, `mlua`) at the same commit. A git dependency does not carry its `[patch]`, so these must move together. `luau0-src` is held at the version zgx locks: a newer Luau writes bytecode that mlua-sys 0.13 refuses to load. Upgrades are deliberate.

## Decoding policy

Every enum that Discord may extend has an `Unknown(i64)` or `Unknown(String)` variant. Collections decode element by element: one bad element is dropped and logged once per kind, never the whole payload. Snowflakes are `u64` behind a newtype that serializes as a string.

## Security boundaries

- Token: `Token` newtype, not `Clone`, not `Debug` printable, zeroized on drop, stored via `keyring`. Plaintext only behind the explicit `storage.plaintext_token` setting, never as a fallback. Secrets, pruning and sign-out: `docs/storage.md`.
- Network: HTTPS only. Redirects drop the Authorization header. Response bodies are capped.
- Hooks: whatever implements `SessionHooks` is untrusted. A failing hook never stops the session.

## Platform

`crates/platform` has no UI framework dependency and targets Windows only. Each module wraps one OS integration behind a plain API.

- `tray-icon` on Windows needs the UI thread's event loop to pump window messages. The desktop app creates the `Tray` inside zgx's run closure, on the UI thread, and keeps it until the window closes. Its events arrive through a callback on that thread.
- `Hotkeys` has the same rule: create it on the UI thread.
- `Tray` and `Hotkeys` install process-global event handlers, so only one of each can exist at a time.
- Single instance and deep links are zgx's: `Application::application_id` claims a named pipe and sets the process AppUserModelID. `instance::is_running` probes that pipe so a second launch can hand over before it starts a session.
- Notifications, clipboard, autostart and the URL scheme work from any thread.