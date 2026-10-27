# zgx spike findings

Tickets 03 and 04 put a zgx window with a Luau interface on the core. The window shows the server rail, the channel list, the messages of the open channel as they arrive, and the signed-in user's avatar. The avatar is fetched and decoded in Rust and handed to zgx as a host image. This page records what we measured and what changes for later tickets. All numbers come from one Windows 11 machine with 16 logical cores. Other lanes were building in parallel during most measurements, so treat the times as upper bounds.

## What the spike window does

- `bridge::Sidebar` projects the guild list and the selected guild's channels. `bridge::Timeline` projects the open channel: the last `WINDOW` (50) messages as id, author display name, plain content and a UTC `YYYY-MM-DD HH:MM` timestamp. It re-projects only when `Changes::messages` names the open channel, or `Changes::members` names its guild. `Timeline::select` returns `Command::SelectChannel`, and the session loads history through its usual path.
- `bridge::load_avatar` loads `cdn/avatars/{id}/{hash}.webp?size=N` through `media::MediaService`. The service decodes with the `image` crate's WebP decoder on a blocking thread and caches on disk and in memory. N is the avatar's 32 points times the display scale, rounded up to a power of two (`bridge::device_pixels`). The RGBA result becomes a `zgx::image::ImageData` and is published with `ScriptHost::image("avatar", ..)`. Luau shows it with `ui.image { src = "host:avatar" }`. zgx's own remote loader is not used.
- `kyoko --mock --chatter N` makes mockcord post into the open channel every N seconds. Mockcord serves deterministic lossless WebP avatars at `/cdn/avatars/{user}/{hash}.webp?size=N`.
- `kyoko --open-busiest` opens the readable text channel with the newest message across all guilds once the guilds arrive. It is for look-only runs, where nobody should read the screen.
- Debug logs added for the comparison record names and counts only, never values: READY's top-level keys, `v` and whether `auth_token` came; the names of cookies Discord sets; the scraped build number; and how many messages each timeline publish appended.

## Builds

| Build | Time | Source |
|---|---|---|
| Cold debug build of the workspace | about 19.5 min | 03 |
| Warm full rebuild | 12 to 13 min | 03 |
| Warm incremental, app crate only, link.exe | 4 to 5 s | 03 |
| Bridge tests after a lockfile change (deps, mockcord, media, session, bridge) | 3 min 3 s | 04 |
| Desktop after the bridge change (`zgx-script` rebuilt too) | 3 min 55 s | 04 |
| Desktop after a `ui.rs`-only change | 38 s to 1 min 37 s | 04 |
| `cargo clippy --workspace --all-targets`, warm | 5 to 20 s | 04 |
| `cargo test --workspace --locked`, warm | about 2 min 30 s | 04 |

`zgx-script` rebuilt whenever the build had last run from the other shell. Keep cargo in one shell, as 03 found.

The debug `kyoko.exe` is 57.7 MB (54 MB in 03). The release `kyoko.exe` is 51.2 MB, with debug info stripped but symbols kept. One release build of the app took 7 min 28 s with a partly warm sccache.

## Memory and CPU (debug build)

| Run | Working set | Private | CPU, share of one core |
|---|---|---|---|
| Mockcord, idle, no channel open | 188.8 MB | 190.9 MB | 0.16 % |
| Mockcord, channel open, one message every 3 s | 196.7 MB | 213.9 MB | 0.10 % |
| Mockcord, channel open, one message every 2 s | 195.5 MB | 195.6 MB | 0.68 % |
| Real Discord, run 1, 8 guilds, channel open, idle | 147.0 MB | 143.4 MB | 0.16 % |
| Real Discord, run 2, same account, channel open, idle | 167.6 MB | 161.1 MB | 0.08 % |
| Release, mockcord, idle, no channel open | 197.7 MB | 191.6 MB | 0.05 % |
| Release, mockcord, channel open, one message every 2 s | 188.9 MB | 192.4 MB | 0.21 % |

Mockcord runs in-process under `--mock`, so the mock figures include it. The real runs stayed flat for the 3.5 minutes they were watched. Release uses about as much memory as debug, so the figure is set by the window, the GPU surfaces and fonts rather than by the code; release mainly saves CPU.

## Bridge behaviour

- **Batching.** The session pushes one `Changes` per applied event into `bridge::Inbox`. The UI task takes everything queued at each wake, merges it into one `Changes`, projects once and sets the Luau shared values inside one `runtime.batch`. Member-list bursts (below) therefore collapse into one projection per wake.
- **Latency.** With `--chatter 2` on mockcord, 36 posted messages produced 36 timeline publishes, each with exactly one appended message. Time from mockcord's post to the timeline publish on the UI thread had a median of 1.4 ms, a minimum of 0.9 ms and a maximum of 4.0 ms. This includes the gateway round trip over loopback, the state reducer and the projection. It excludes the paint, which follows on the next frame.
- **Drops and duplicates.** None. Bridge tests check that ids stay unique and in order across history plus live messages, and that a message in another channel leaves the open one untouched.
- **Avatar.** From the request to the image being handed to the window took 72 to 125 ms on mockcord and 207 and 306 ms against the real CDN, for a 32 px WebP. The display scale read from `WindowInfo` was 1.0 on this machine, so HiDPI sizing is untested.
- **Read acks.** During the real runs, before ticket 10 merged, opening a channel sent one immediate `POST /channels/{id}/messages/{last}/ack` and incoming messages were never acked. Both runs sent exactly one ack. Ticket 10 now owns acks: the session debounces them, including for new messages in the viewed channel (see `docs/rest.md`). The spike window does not yet report `SetViewing`, because zgx gives the host no window focus or minimise signal, so the session treats the window as always viewing.
- **Projection cost.** `Timeline::apply` rebuilds the 50 entries and compares them with the previous ones. Each publish replaces the whole Luau list. That is fine at spike scale; ticket 25 replaces it.

## zgx issues met

- **Exit hang.** `Application::run` sometimes did not return after the window closed, after `on_closed` had run and the session had stopped. Ticket 20 found the cause and works around it; see [Exit hang](#exit-hang) below.
- **Rounding.** `rounded = 16` on `ui.image` does not clip the image. The avatar shows square.
- **Helper windows.** Windows titled `Kyoko` include zero-size zgx helper windows, as 03 noted. Scripts that look for the window must take the largest match. A window minimised by a stray click needs `ShowWindow` before its client rect is non-zero.
- `follow = true` on `ui.scroll` keeps the message list pinned to the newest message as messages arrive, with no extra code.
- Host images need exactly `width * height * 4` RGBA bytes. `media`'s `DecodedImage` fits directly, and the first frame is used for animated images.
- **Cursors.** `Scene::cursor_at` takes the topmost hit and ignores `pointer_events = "none"`, which input dispatch honours (`NodeInput::pass_through`), so a full-window pass-through node hides every explicit cursor under it. Kyoko keeps such nodes out of the hit test while they show nothing (v1 86, `docs/ui.md` "Cursors and the caret"); upstream fix: skip pass-through nodes in `cursor_at`.
- **Caret.** Editors never blink their caret. Kyoko blinks it from the host through `Ui::focused_editor` and the public `EditorHandle::caret` node (v1 86); zgx is unchanged.

## Exit hang

**Symptom.** After the last window closed, `on_closed` ran and kyoko's cleanup finished, but `Application::run` did not return and the process stayed alive with no window. Posting any message to the UI thread (`PostThreadMessageW(thread, WM_NULL)`) made it return at once.

**Cause (zgx and its winit fork, `fc7d0f1`).** When the last window closes, zgx calls `event_loop.exit()` from `about_to_wait` (`WindowManager::about_to_wait_turn` in `zgx-desktop/src/application.rs`) and returns without changing the control flow. On Windows, winit's `EventLoop::wait_for_messages` (`vendor/winit/src/platform_impl/windows/event_loop.rs`) dispatches `AboutToWait` through `runner.prepare_wait()` and then goes straight into `MsgWaitForMultipleObjectsEx` with that control flow. It only checks the exit code after the wait returns. With `ControlFlow::Wait`, or a far `WaitUntil`, and no message arriving, the thread sleeps forever. Usually some message arrives right after the window is destroyed, which is why only some closes hang. Upstream fix: zgx should `set_control_flow(ControlFlow::Poll)` when it calls `exit()`, or winit should skip the wait once an exit is requested.

**Workaround in kyoko.** `frame::wake_until_loop_ends` starts from `on_closed`. It posts `WM_NULL` to the UI thread every 50 ms until `run` returns, for at most 10 seconds. Every close path ends in `on_closed`: the title bar's close button, Alt+F4, the taskbar, `WM_CLOSE` from the parity capture, and the tray's Quit. The waker thread exists only while the app is exiting, so it costs nothing while kyoko runs.

**Measurements** (debug build, mock, capture mode so close-to-tray is off, `WM_CLOSE` posted to the window 4 s after it appears, 15 s allowed to exit):

| Build | Runs | Process still alive |
|---|---|---|
| Waker disabled | 20 | 2 |
| Waker | 25 | 0 |
| Waker | 19 | 0 |

With the waker, close to `run` returning took 0.10 to 0.51 s, median 0.11 s.

**Minimal repro for an upstream report** (a bare zgx window, no kyoko code; about one close in ten to one in three stays alive, depending on load):

```rust
use zgx::compose::prelude::*;
use zgx_desktop::{Application, WindowOptions};

fn main() {
	Application::new()
		.window(WindowOptions { title: "repro".into(), width: 800., height: 600., ..Default::default() })
		.run(|cx| cx.render(column().w_full().h_full().child(text("close me"))))
		.unwrap();
	eprintln!("window loop ended");
}
```

Run it, close the window (or post `WM_CLOSE` to it) and repeat. In a hung run the window is gone, `window loop ended` never prints, the main thread waits in `NtUserMsgWaitForMultipleObjectsEx`, and posting `WM_NULL` to the thread ends it.

An early theory blamed winit buffering a `WM_CLOSE` dispatched from a nested message loop. That came from a test script that sometimes posted to the window before it was ready. Once the script picked the window at close time, every close was dispatched, and the only failure left was the exit stall above.

## Real Discord runs

There were two look-only runs, each under five minutes, with the path as in 03: `client_identity`, `build_source: Some(discord_build_source())`, `remember_account: false`, and a scratch data directory. Credential Manager was unchanged afterwards.

- **Connection.** Both runs reached READY on gateway `v=9`. There was no captcha, no 401, no 4004, no close code and no warning.
- **What opened.** The account has 8 guilds. `--open-busiest` opened a text channel in a guild that listed 43 channels in run 1 and 35 in run 2. Its 50 most recent messages loaded through one `GET /channels/{id}/messages`.
- **Live messages.** None arrived in the open channel in either run, about 3.5 minutes each. Run 1 saw one MESSAGE_CREATE and five TYPING_START in other channels. Live arrival against real Discord is still unconfirmed; it is covered on mockcord and by the bridge test.
- **Dispatches.** Over about four minutes: READY, READY_SUPPLEMENTAL, SESSIONS_REPLACE, THREAD_LIST_SYNC and MESSAGE_ACK once each. GUILD_MEMBER_LIST_UPDATE arrived 60 times in run 1 and 76 times in run 2, from the guild subscription the session sends when a channel opens.
- **REST.** Each run made exactly one messages GET and one ack POST. The avatar came from the CDN through `media`, outside the REST client.
- **Build number.** The scrape found 649389 in both runs, against the old fallback of 647615. The public Discord-Datamining repository showed canary build 650154 on 2026-10-25.
- **Cookies.** Fetching `/app` set `__dcfduid` and `__sdcfduid`. API responses set `_cfuvid`.
- **READY keys.** `_trace, ad_personalization_toggles_disabled, analytics_token, apex_experiments, api_code_version, auth, auth_session_id_hash, broadcaster_user_ids, connected_accounts, consents, country_code, experiments, explicit_content_scan_version, friend_suggestion_count, game_relationships, geo_ordered_rtc_regions, guild_experiments, guild_join_requests, guilds, linked_users, merged_members, notification_settings, pending_payments, private_channels, qos_token, read_state, regional_feature_config, relationships, resume_gateway_url, session_id, session_type, sessions, static_client_session_id, subdivision_code, tutorial, user, user_guild_settings, user_settings, user_settings_proto, users, v`.
  - Compared with the documented list, `notes`, `presences` and `merged_presences` are absent, as capability bits 0 and 5 imply. `auth_token` and `required_action` are absent too.
  - `qos_token` and `subdivision_code` are undocumented extras.
  - `user_settings_proto` arrives even though bit 9 is not advertised.
- **Refreshed token.** `auth_token` was absent in both runs, although bit 8 is advertised. Ticket 08's refresh path is still unexercised against real Discord.

## Real-traffic comparison

References:
- discord-userdoccers, served as docs.discord.food and docs.discord.sex, read at master on 2026-10-25:
  - [Using the gateway](https://docs.discord.food/gateway/using-gateway), last changed 2026-08-13
  - [Gateway events](https://docs.discord.food/gateway/gateway-events), last changed 2026-08-27
  - [Reference](https://docs.discord.food/reference), last changed 2026-08-18
  - [Experiments](https://docs.discord.food/topics/experiments)
- Chrome's [release schedule](https://chromiumdash.appspot.com/schedule): M156 reached stable on 2026-10-20, and 156.0.8078.49 shipped on 2026-10-22. M157 early stable came out on 2026-10-22, with full stable due on 2026-11-03.

The docs do not describe the web client's exact current traffic. Their IDENTIFY example is generic. Where they are silent, kyoko follows what Chrome itself sends.

| Item | Kyoko sends | Reference | Recommendation |
|---|---|---|---|
| Chrome major | 152 in the UA, `browser_version` and `sec-ch-ua` | Stable is 156; 157 from 2026-11-03 | Move to 156 (or 157 after 3 November), and recompute the `sec-ch-ua` GREASE brand for that major, since Chrome derives it from the major. More than one line, so left for a follow-up. |
| Fallback build number | 647615 | Real scrape 649389, twice | Changed to 649389 in this ticket (one line). The scrape works, so the fallback only matters offline. |
| Build scrape | `build_number:"` and related patterns on `/app` | Found 649389 within the 5 s cap | No change. |
| Gateway URL | `v=9&encoding=json&compress=zlib-stream` | Same as the docs example; `zstd-stream` is also documented | No change. |
| Capabilities | 509 (bits 0, 2 to 8) | The docs example uses 1734653 (adds 9, 10, 12, 13, 14 and undocumented 17, 19, 20), not stated as the web client's value | Keep 509. Bit 8 brought no `auth_token`. Bits 13 and 14 need the batched-reaction and passive-update parsers first (13, 16). Bit 9 can wait for settings-proto work, since the proto already arrives. Don't add undocumented bits. |
| IDENTIFY key order | token, capabilities, properties, presence, compress, large_threshold, client_state | Docs example: token, properties, compress, presence, capabilities, client_state | Reorder to the docs order and drop `large_threshold`, whose user default is already 250. Low risk; do it with 19. |
| Presence | `status: "online"`, `since: null` | Docs: users send `"unknown"` on a fresh IDENTIFY; the example uses `since: 0` | Send `"unknown"` and `since: 0`, and let the settings status apply (13, 30). |
| `client_state` | `{guild_versions: {}}` | The example adds `api_code_version: 0`; READY returns an `api_code_version` | Add `api_code_version` and echo READY's value with the rest of the versioned client state (13). |
| `launch_signature` | Omitted | In the docs web example, between `client_launch_id` and `client_heartbeat_session_id`, with a public generator | Add it, generated as the docs describe (19). |
| `client_app_state` | `"focused"` | Documented, but not in the web example | Keep for now; review with 19. |
| `client_heartbeat_session_id` | One UUID per launch | Regenerated every 30 minutes | Rotate every 30 minutes (19). |
| `installation_id` | Only as `X-Installation-ID` | Also sent in IDENTIFY, outside `X-Super-Properties` | Add it to IDENTIFY once known (13). |
| Other properties | os, browser, device, locale, UA, version, os_version, referrers, release_channel, build, event source, has_client_mods, launch id | Same set; `search_engine_current` and `mp_keyword_current` appear only for search referrals | No change. |
| REST client headers | `sec-ch-ua*`, `Sec-Fetch-*`, `Priority: u=1, i`, `Accept-Encoding: gzip, deflate, br, zstd` | Not covered by the docs; matches Chrome's fetch defaults | No change beyond the Chrome bump. |
| Discord headers | `X-Super-Properties`, `X-Discord-Locale`, `X-Discord-Timezone`, `X-Installation-ID` once known | The first three are documented; locale falls back to `Accept-Language`; `X-Discord-Timezone` is undocumented | No change. |
| Absent headers | No `X-Debug-Options`, `X-Fingerprint` or forced `locale` cookie | `X-Fingerprint` is only for requests before login; `X-Debug-Options` is optional | Correct. `X-Context-Properties` will matter for joins and invites (35). |
| Cookies | Jar filled from responses | `__dcfduid`, `__sdcfduid` and `_cfuvid` were seen | No change; web login (43) should expect these. |
| Avatar URL | `/avatars/{id}/{hash}.webp?size=N`, a power of two | The CDN picks the format from the extension; `?format=` belongs to the media proxy | No change. |
| Media request headers | User agent only | Chrome image requests carry an image `Accept`, `Sec-Fetch-Dest: image` and Referer `https://discord.com/` | Match Chrome's image request headers (18). |

## Changes for later tickets

- **06 / 19, fingerprint.** Bump Chrome to 156/157 together with the GREASE brand. Reorder the IDENTIFY keys and drop `large_threshold`. Add `launch_signature`. Rotate `client_heartbeat_session_id` every 30 minutes. Recheck the fallback build each release; 649389 was current on 2026-10-25.
- **08.** No `auth_token` came in two real READYs with bit 8 advertised, so the refresh path is still unverified against real traffic.
- **10, read acks.** Done on main. The window still has to report `SetViewing(false)` when it loses focus or is minimised, and `SetViewport` from the message list. zgx exposes neither focus nor minimise to the host yet, so this lands with 25, or with 20 if the app frame gets a focus signal first.
- **13, client_state.** Add `api_code_version` and echo READY's value. Send presence `"unknown"` with `since: 0`. Put `installation_id` in IDENTIFY. `user_settings_proto` already arrives without bit 9.
- **14, member lists.** One open guild produced 60 to 76 GUILD_MEMBER_LIST_UPDATE in about four minutes while idle. Coalesce them and keep them off the timeline path. `Timeline::apply` currently re-projects on any member change in the open guild.
- **16 / 13, capabilities.** Bits 13 and 14 stay off until their parsers exist.
- **18, media.**
  - Send Chrome's image request headers.
  - Size avatars from `WindowInfo` scale; only 1.0 has been seen so far, and the signal may not be ready by READY.
  - Animated hashes get a still WebP; animation is ticket 18's job.
  - zgx does not clip host images with `rounded`, so pre-mask the RGBA or ask zgx for clipping.
  - End-to-end avatar time was 200 to 300 ms against the real CDN.
- **20 / 58, app frame and packaging.** Done in 20: the exit hang is explained and worked around (see [Exit hang](#exit-hang)); report it upstream. Rounded image clipping is still open for 22 and 30.
- **25, timeline.** Replace `bridge::Timeline` with the Rust timeline view. Publish diffs instead of the whole list. Show local timestamps instead of UTC. `follow = true` already gives end anchoring.
- **35, community flows.** Joins and invites need `X-Context-Properties`, which kyoko never sends today.
- **43, web login.** The cookies to expect are `__dcfduid` and `__sdcfduid` from `/app`, and `_cfuvid` from the API. The build scrape works from the same jar.
- **45, plugins.** `ScriptHost::image` names and `ui.shared` names are global to the host. Namespace them per plugin before plugins can publish images or state.
