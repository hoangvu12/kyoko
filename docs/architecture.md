# Architecture

Kyoko is a Cargo workspace. Data flows one way: gateway event, state reducer, revision bump, UI re-render of the views whose revision changed. The UI never performs network calls; it sends `Command` values to the session, which owns the REST client and the gateway task.

```
apps/desktop        binary: for now only `kyoko --mock`, a headless session against mockcord; the zgui host replaces it
crates/protocol     typed Discord REST and gateway models, lenient serde decoding
crates/gateway      websocket session: identify, heartbeat, resume, zlib-stream, backoff
crates/rest         HTTP client: rate-limit buckets, 429 retry, Authorization redaction
crates/state        normalized store, permission math, read state, message cache
crates/store        sqlite cache and settings, token in the OS keyring
crates/markdown     Discord markdown parser to AST
crates/media        image and animation decoding, texture cache
crates/session      session core: owns REST, gateway and persistence, takes Commands, emits Changes
crates/login        token, QR and web login
crates/platform     Windows tray, notifications, single instance, hotkeys, clipboard
tools/mockcord      mock Discord REST and gateway server driven by fixtures
tools/xtask         headless-run, theme catalog tooling, release notes
```

Crate graph: `protocol` is the leaf. `rest` and `gateway` depend on `protocol`; `state` on `protocol` and `markdown`; `store` on `protocol`. `session` sits on all of them. `apps/desktop` depends on `session`, `state`, `store`, `protocol` and `mockcord`. None of `protocol`, `rest`, `gateway`, `state`, `store`, `markdown` or `session` has a UI dependency.

## Session hooks

The session has no plugin runtime. `SessionConfig::hooks` takes an optional `Arc<dyn SessionHooks>`; every method has a no-op default:

- `before_send(channel_id, content)` and `before_edit(channel_id, message_id, content)` may return replacement text. A rewritten send is shown as altered by `name()`.
- `allow_typing(channel_id)` can veto the typing indicator.
- `allow_notification(message)` is exposed as `Session::should_notify` for the UI to ask before it raises a notification.
- `on_dispatch(event)` observes every gateway dispatch after it is applied to state. It runs on the session task, so implementations must return quickly and queue any real work.

Text hooks run on a blocking worker. Each call is wrapped in `catch_unwind`; an error or panic is logged with the hook name only, never the message content, and the session continues with the unmodified value.

## Threads

- UI thread owns the window and all view entities.
- A tokio runtime on background threads owns the gateway socket, REST client and sqlite writer.
- Crossing: `tokio::sync::mpsc` for commands into the session, a `watch`-style revision counter plus `Arc` snapshots out to the UI. No lock is held across a frame.

## Decoding policy

Every enum that Discord may extend has an `Unknown(i64)` or `Unknown(String)` variant. Collections decode element by element: one bad element is dropped and logged once per kind, never the whole payload. Snowflakes are `u64` behind a newtype that serializes as a string.

## Security boundaries

- Token: `Token` newtype, not `Clone`, not `Debug` printable, zeroized on drop, stored via `keyring`. Plaintext fallback only behind the explicit `storage.plaintext_token` setting.
- Network: HTTPS only. Redirects drop the Authorization header. Response bodies are capped.
- Hooks: whatever implements `SessionHooks` is untrusted. A failing hook never stops the session.

## Platform

`crates/platform` has no UI framework dependency and targets Windows only. Each module wraps one OS integration behind a plain API.

- `tray-icon` on Windows needs the UI thread's event loop to pump window messages. The desktop app must create the `Tray` on the UI thread after the event loop exists, and keep it alive for the process lifetime. The `Receiver<TrayEvent>` can be drained from any thread.
- `Hotkeys` has the same rule: create it on the UI thread.
- `Tray` and `Hotkeys` install process-global event handlers, so each can be created once per process.
- `single_instance::acquire` returns `AlreadyRunning` when another instance holds the lock. The second launch should call `notify_existing` with its CLI args (deep link) and exit.
- On Windows, toasts need an AppUserModelID: call `notifications::set_app_user_model_id` at startup and pass the same id as `Notification::app_id`.
