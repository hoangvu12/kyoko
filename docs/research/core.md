# Lightweight Discord client on zgui: core research

Date: 2026-10-25. Scope: everything except the UI. The UI discussion comes next and builds on this.

Sources read:
- `zeronsh/zgui` @ `a9dc282` (2026-10-24, public, MIT). Cloned and read only, not built.
- `chojs23/concord` @ `b312f18` (v2.6.3, GPL-3.0-only). Cloned and read only, not built.
- Discord's official developer docs, ToS and Support pages.
- Community user-API docs (docs.discord.food, discord.py-self), labelled as community wherever they're used.
- Rust crate sources (twilight, serenity, keyring, …).

Every claim in Parts A and B carries its own citation (`path:line @commit` or a URL) and a read/inferred mark. This top section only synthesizes them. Where it states something new, it says so.

---

## Verdict

**Feasible. Go ahead, on four conditions.**

1. **The core is our own Rust code, not a library.** No crate can log in as a user account: twilight and serenity force a `Bot ` prefix and send intents, twilight's `Ready` needs a bot-only field, and serenity dropped self-bot support in 2017 (B §10.3). Concord shows the whole job can be done, but it's GPL-3.0. Copying it would make our app GPL, so we use it as a behaviour reference only (B §10.2).
2. **zgui handles the window, drawing and Luau UI. Everything that touches the network or the OS is ours, in Rust.** That means:
   - gateway WebSocket and REST
   - image fetch and decode, including WebP, which zgui lacks and Discord's CDN serves
   - a bounded image cache
   - an event queue into the UI thread
   - notifications, tray, image clipboard, file drop
   (A §7, A §5, A §9)
3. **The message list needs Rust work no matter how the UI is designed.** zgui's Luau `ui.list` only renders visible rows, but heights have to be supplied by us, rows are keyed by position, and it can't stick to the bottom. The stick-to-bottom list is Rust-only, and neither list keeps your place when older history is inserted above (A §4). This is the main interface between core and UI, so it's the first UI topic.
4. **Treat account risk as a design constraint.**
   - Pass for the web client: live build number, consistent Chrome UA and super-properties, cookie jar, per-channel Referer.
   - Pace requests and never auto-retry a captcha.
   - Default to token login.
   - Add Concord-style locks on risky DMs.

   This lowers the risk but doesn't remove it. Discord's ToS and self-bot policy still apply (B §7).

**Size (B §11):**
- Text-only core: about 14k–20k lines of Rust plus tests, roughly 10–16 weeks for one experienced engineer. A working gateway/state/REST skeleton could exist in about 4–6 weeks.
- Voice and screen share: excluded. Concord's voice code alone is about 32k lines, roughly twice the whole text MVP.

**Memory:** don't expect Concord's 20–40 MB. zgui's own idle baselines are about 116 MB on macOS and 103–108 MiB RSS on Linux with a software GPU. There are no Windows memory figures (A §8). The realistic target is "much lighter than the Electron client", not "as light as a TUI".

---

## Proposed core architecture

My synthesis of A and B. This design was not tested.

```
┌─ background (tokio, own threads) ─────────────────────────────────────────┐
│ auth      token (keyring → Windows Credential Manager), later pw+MFA / QR   │
│ fingerprint  live build no., Chrome UA, X-Super-Properties, cookie jar,     │
│              installation id, per-channel Referer                           │
│ gateway   actor: zlib-stream inflate, HELLO/heartbeat+ACK, IDENTIFY         │
│           (capabilities 253, client_state, no intents) / RESUME, fatal      │
│           close codes, 120/60s two-priority send window, op 37 debounce,    │
│           op 8 scheduler                                                    │
│ rest      api/v9, bucket learning from X-RateLimit-*, mutation pacing,      │
│           403 breaker, stop-on-401, captcha → stop + tell user              │
│ state     Arc-per-area copy-on-write + per-area revisions; 200 msgs/channel,│
│           N warm channels, cold = metadata only; lenient serde at edges     │
│ media     CDN/proxy URL builder (device-pixel sizes, webp), pooled fetch,   │
│           decode (PNG/JPEG/GIF/WebP), byte-budgeted LRU                     │
└──────────────┬──────────────────────────────────────────────────────────────┘
               │ events: own Inbox (Mutex<Vec<Event>> + Waker)    ▲ commands
               ▼          (zgui::task::latest coalesces: wrong)   │ (ui.invoke →
┌─ UI thread (zgui LocalExecutor) ──────────────────────────────┐ │  host.action /
│ drain inbox → read changed areas → project only what's shown  │ │  async_action →
│ → Shared::set per area (reconciles by `id`, unchanged rows    │ │  channel to
│   skip) ; images → ScriptHost::image("host:<key>")            │ │  tokio)
│ Luau: view only                                               ├─┘
└───────────────────────────────────────────────────────────────┘
```

The key rules that come out of the research:

- **Hand data to the UI thread with a queue, not `latest`.**
  - zgui `Value`s are `Rc`-based and can only be built on the UI thread (A §3).
  - `zgui::task::latest` keeps only the newest value, so events sent through it are lost (A §3).
  - Use our own inbox, as zgui's `zeron_rpc.rs` example does, or use `latest` purely as a "something changed" ping while the real state stays in the Rust store.
- **Publish projections, not the whole store.** `Shared::set` reconciles in O(n). Publish only the active channel's message window, the visible guild/channel tree and the visible member ranges, never the full cache.
- **tokio and zgui's executor don't mix directly.** `fetcher` and `async_action` futures run on zgui's single-threaded executor. Cross over with channels (A §3).
- **Images bypass `ui.image { src = "https://…" }`.** Its cache never evicts and it spawns one thread per decode (A §5). Rust does fetch, decode and LRU, then hands images over as `host:` images.
- **Disk persistence:**
  - Save `client_state` versions and a small snapshot, so the next READY is smaller (B §2.3, B §8). This change to Concord is my suggestion.
  - Don't use `ui.persist` for caches; it writes one JSON file per key (A §7).

**Protocol decisions taken from the research (B):**
- Gateway `v=9&encoding=json&compress=zlib-stream`, to match what Concord says the browser uses.
- Allow inbound frames up to 64 MiB, because user READY payloads can exceed 16 MiB.
- Keep outbound gateway payloads ≤ 4096 bytes.
- Use op 37 for subscriptions (op 14 is deprecated), with member ranges of 100 rows and at most 3–4 ranges.
- Learn REST rate limits from response headers; don't hard-code them.

---

## Risks, ranked

| # | Risk | Where | Mitigation |
|---|---|---|---|
| 1 | Account action by Discord (ToS / self-bot policy, automated enforcement) | B §7 | Token login default; web-client fingerprint; pacing; DM safety locks; no automation features; captcha → stop |
| 2 | zgui immaturity: 0.1, about one month public, most commits by a bot, breaking default changes | A §1 | Pin a commit; copy its 9 `[patch.crates-io]` forks; upgrade on purpose, not by drift |
| 3 | Windows is the least-tested zgui platform (CI is build + unit tests + WARP; real-input tests are run by hand) | A §2 | Spike on Windows first (see next steps) |
| 4 | Message list: no Luau primitive for measured, keyed, stick-to-bottom rows with prepend anchoring | A §4 | Build in Rust; decide the approach in the UI discussion |
| 5 | Fingerprint drift (Chrome version, build-number scrape) | B §7 | Fetch the live build number; keep fingerprint data in one module that's easy to update |
| 6 | Undocumented protocol churn (op 37, capabilities, READY shape) | B §1–3 | Lenient parsing; raw-dispatch debug log |
| 7 | GPL contamination from Concord | B §10.2 | Clean-room from this document plus community docs; never paste code |
| 8 | Token is a full account credential | B §6.5 | Credential Manager only; no plaintext fallback on Windows |

---

## Open decisions (for the user)

1. **License.**
   - MIT, written clean-room with Concord used only as a behaviour reference (recommended).
   - GPL-3.0, so Concord code can be reused directly.
2. **Where it lives.** A new repo, separate from Roboco, is the obvious choice, but it needs a name.
3. **Login methods for the MVP.** Token only (recommended), or also email/password + MFA and QR (+1–2 weeks, and more captcha exposure).
4. **zgui tracking.** Pin `a9dc282`, or another commit, and decide how often to upgrade.
5. **Spike before committing.** I recommend about one week to prove the risky parts on Windows:
   - zgui builds with the patches
   - token login and gateway connect
   - READY parsed into the store
   - the inbox bridge feeding a Luau guild/channel list
   - live `MESSAGE_CREATE` showing up
   - one WebP avatar via `host:` images

---

## How this document is organized

- **Part A: zgui as the runtime.** Platforms, the Luau bridge, threading, lists, images, text, networking and system services, performance. It opens with a verdict table.
- **Part B: Discord user-account core.**
  - gateway; READY; lazy loading and member lists
  - minimum event set; REST and rate limits
  - auth; fingerprint and safety
  - state and cache; media
  - build vs reuse; effort estimate
  - It opens with its own summary.

Each part keeps its own source list at the end.

---

# Part A: zgui as the runtime

## zgui as the runtime for a lightweight Discord client: what it gives, where it falls short

Source read: `https://github.com/zeronsh/zgui` @ **`a9dc28246170b4099a414191ca6fc172586b0f9c`** ("Styles mark the fields they set, and compare only those", 2026-10-24). It was cloned read-only to `%TEMP%\zgui-src` (`/tmp/zgui-src`). Nothing was built or run. The repo is public: a plain `git clone` worked and `gh repo view` reports `"visibility":"PUBLIC"`. CI run [40113007210](https://github.com/zeronsh/zgui/actions/runs/40113007210) passed on this exact commit.

Citation format: `path:line @a9dc282`. Anything I did not read directly is marked **inferred**.

The zterm reference copy (`zgui-zterm-src`, pinned at `c804b803`) is an ancestor of HEAD, **287 commits behind** (`git rev-list --count c804b803..HEAD`). Facts below come from HEAD, not from the pin.

---

### Fit-verdict table

| # | Area | Verdict | One-line evidence |
|---|---|---|---|
| 1 | Project basics | MIT, 0.1, git-only, **independent of GPUI/zui** | Own wgpu renderer + winit; GPUI appears only as a benchmark comparison (`comparisons/gpui/Cargo.toml:6 @a9dc282`, excluded from the workspace, `Cargo.toml:3-14 @a9dc282`) |
| 2 | Windows | **Available, with gaps** | DX12 renderer, WebView2, IME, spellcheck, popups, single instance (`docs/windows.md @a9dc282`). CI on Windows is clippy + unit tests + WARP pixel tests only (`.github/workflows/ci.yml:262-289 @a9dc282`). `docs/ROADMAP.md:5` still calls Windows a "portability target" |
| 2 | Linux | **Available** | Wayland + X11, native probes run in CI (`docs/linux.md:1-6`, `ci.yml:143-149 @a9dc282`) |
| 3 | Luau engine + bridge | **Available** | mlua 0.12 `luau` (vendored patch), per-script sandboxed VM, Rust-owned keyed state, reconciling `Shared` |
| 3 | High-rate stream, background thread → UI | **Partial** | `zgui::task::latest` exists but **coalesces, keeping only the latest value**. A chat event queue must be your own `Mutex<Vec>` + `Waker`, as the `zeron` example does. No public background executor |
| 4 | Virtualized chat list in Luau | **Partial** | `ui.list` virtualizes, but rows are fixed-height or have **app-supplied heights (never measured)**, are keyed **by index**, and have **no `follow`** |
| 4 | Stick-to-bottom | **Partial** | `ui.scroll { follow = true }` is non-virtualized (warns past 1,000 `each` rows). Rust `VariableHeights::anchor_end()` is not exposed to Luau |
| 4 | Scroll anchoring on prepend (older history) | **Missing — build in Rust** | The Rust variable list anchors the first visible row only when the row count is unchanged. A count change keeps the pixel offset (**inferred** from code) |
| 5 | Images from URL / bytes | **Available** | `ui.image { src = "https://…" }` goes through the host fetcher. `host:<name>` takes Rust-supplied decoded `ImageData` |
| 5 | Animated GIF | **Partial (Rust only)** | `animated_image` + `decode_gif` exist in Rust. The Luau `ui.image` path decodes stills only (PNG/JPEG) |
| 5 | WebP / animated WebP / APNG | **Missing — build in Rust** | No decoder in the tree (grep finds no WebP code) |
| 5 | Image cache for many avatars | **Partial** | The Luau remote-image cache is unbounded "for the host's life", with one OS thread per decode. The Rust `ImageCache` is a bounded LRU but sits in a separate API |
| 5 | Inline image in rich text (custom emoji) | **Partial** | A span takes one `icon` (square, before its text, file/`https://`/SVG). No standalone inline image run, no animation |
| 6 | Composer (multiline, IME, undo, spellcheck) | **Available** | `ui.textarea` / `input { multiline }`, IME smoke-tested on Windows (Korean). Known gap: no IME reconversion |
| 6 | Selectable / copyable read-only text | **Available** | `selectable = true`, across nodes and recycled `ui.list` rows, copies as text + HTML |
| 6 | Markdown | **Partial** | `ui.markdown` is a parser into blocks, incremental with `key`. No renderer, and no Discord dialect (spoilers, mentions, timestamps, custom emoji) |
| 7 | HTTP (`ui.fetch`) | **Missing built-in — bring your own** | `ScriptHost::fetcher` is required. "zgui adds no HTTP dependency". `zgui-run` installs none (**inferred** from grep) |
| 7 | WebSocket | **Missing — build in Rust** | Only a dev-dependency (`tungstenite`) for the `zeron` example |
| 7 | OS notifications, tray, audio playback | **Missing — build in Rust** | No code found (grep) |
| 7 | Clipboard | **Partial** | Text read/write in Luau. Rust adds text+HTML. **No image clipboard** |
| 7 | File dialogs, open URL, single instance, URL scheme | **Available** | `ui.open_files`/`save_file`, `Platform::open_url`, `Application::application_id` |
| 7 | OS file drop (attachments) | **Partial (Rust only)** | `View::on_files_drop` exists. Luau `on_drop` takes only zgui's own drag payloads |
| 8 | Perf data | **Partial** | macOS heavy-scene numbers vs GPUI, and Windows latency/pacing numbers on an RTX 4090. **No Windows/Linux memory comparison vs GPUI** |

---

### 1. Project basics

- **License:** MIT (`LICENSE:1 @a9dc282`; `Cargo.toml` `license = "MIT"` @a9dc282; `gh repo view` → `licenseInfo.key = "mit"`).
- **Maturity:**
  - "It is young (0.1, used from git) and its API still changes" (`README.md:3 @a9dc282`).
  - "Nothing is published yet" (`docs/releasing.md:3 @a9dc282`). The crates.io name `zgui` is taken by an unrelated project (`docs/releasing.md`, "Before the first publish" @a9dc282).
  - The changelog lists breaking default changes (stretch/shrink, RTL, `cmd`→Ctrl) under "Unreleased (October 2026)" (`CHANGELOG.md:5-20 @a9dc282`).
- **Consumers must copy 9 patched forks.** Your workspace has to repeat zgui's `[patch.crates-io]` entries: cosmic-text, taffy, wgpu-hal, four AccessKit adapters, winit and mlua. Without them, zgui "doesn't compile … or behaves differently" (`docs/getting-started.md:9-28 @a9dc282`; `Cargo.toml:69-92 @a9dc282`).
- **MSRV:** `rust-version = "1.95"` (`Cargo.toml:25 @a9dc282`).
- **Crate layout:** workspace members `zgui`, `zgui-components`, `zgui-desktop`, `zgui-ghostty-sys`, `zgui-gpu`, `zgui-script`, `zgui-run`, plus `comparisons/{workload,demo}` (`Cargo.toml:2 @a9dc282`).
  - `zgui` (core): reactive signals, retained scene, layout (taffy), input, text editing, accessibility. Deps: taffy, unicode-*, stacker, smallvec (`crates/zgui/Cargo.toml @a9dc282`).
  - `zgui-gpu`: wgpu 29 renderer (DX12/Metal; Vulkan/GL on Linux), cosmic-text, image/resvg/gif (`crates/zgui-gpu/Cargo.toml:37-58 @a9dc282`).
  - `zgui-desktop`: winit 0.30.13 host, arboard clipboard, rfd dialogs, AccessKit, platform code (`crates/zgui-desktop/Cargo.toml:47-56 @a9dc282`).
  - `zgui-script`: Luau via mlua (`crates/zgui-script/Cargo.toml:34 @a9dc282`), plus pulldown-cmark for `ui.markdown`.
  - `zgui-components`: data grid, tree, tabs, toasts, palette and the like.
  - `zgui-run`: runs a script directory as an app.
  - `zgui-ghostty-sys`: Ghostty terminal FFI (macOS).
- **Rendering backend:** zgui's own retained renderer on wgpu, damage-tracked (`README.md:9 @a9dc282`; `docs/architecture.md:23-37 @a9dc282`).
- **Windowing:** winit (patched) (`Cargo.toml:86-88 @a9dc282`).
- **GPUI/zui:** zgui does **not** depend on GPUI or zui.
  - `gpui = "=0.2.2"` appears only in `comparisons/gpui/Cargo.toml:6 @a9dc282`, a separate benchmark workspace excluded from the main one (`Cargo.toml:3-14 @a9dc282`).
  - `Cargo.lock` has no `gpui`/`zui` entries (grep @a9dc282).
  - It is therefore unrelated to `C:\Users\ADMIN\Desktop\nguyenvu\zui`.
- **Commit activity:**
  - 498 commits on `main`, from 2026-09-26 (`92fd6a4 "zgui"`) to 2026-10-24 (`git log`).
  - 468 of them landed 2026-10-20…24 (`git log --date=short | uniq -c`).
  - Authors: `zeronsh[bot]` 414, Wing 69, wing-anara 6, Sina Khalili 6, grahamwinter 3.
  - About 120 live remote branches, about 50 of them `windows/*`. Repo created 2026-09-26, 5 stars (`gh repo view`).
  - Reading: very young, very high velocity, mostly bot-authored commits (**inferred**: an agent-driven workflow).

### 2. Platforms

#### Windows

**Build requirements.** Stable Rust, `x86_64-pc-windows-msvc`, and Visual Studio Build Tools (Desktop C++ + Windows SDK). "No additional Windows-only dependency is needed" (`docs/windows.md:3-15 @a9dc282`).
- Luau is C++ compiled by mlua's build, so MSVC is required (**inferred** from mlua `luau` feature).
- ARM64 builds, but no CI job runs it (`docs/windows.md:17-30`).
- Set `LUAU_CXXFLAGS=-DNDEBUG` for release builds. Without it scripts run about 20% slower (`docs/scripting.md:460-468`; `.cargo/config.toml @a9dc282`).
- No Zig is needed unless you enable the macOS-only `ghostty` feature (`crates/zgui-desktop/Cargo.toml:30`).
- `vulkan` is an optional feature (`docs/windows.md:53-60`).

**What works on Windows** (all `docs/windows.md @a9dc282`):
- **Rendering:** wgpu on Direct3D 12 only (WARP fallback), DXGI flip model, compositor-clock pacing. Keystroke-to-screen latency is 12 ms median while animating (`:51-75`).
- **Window chrome:** Win11 inset title bar with snap layouts, Mica/Acrylic glass (`:107-130`), executable icon (`:132-136`).
- **Popups:** menus and popovers in owned popup windows (`:142-168`).
- **Drag and drop:** OLE drag-out (`:170-193`).
- **System settings:** accent, high contrast, text size, reduced motion and scrollbars (`:195-220`).
- **WebView2:** behind the `webview2` feature (`:222-271`).
- **Touch and pen:** supported (`:273-283`).
- **Single instance via named pipe and AppUserModelID:** `:285-291`; `docs/platform-services.md:126-136`.
- **Shipping notes:** crt-static, size 12.1 MiB for `component_workload`, signing, MSIX (`:293-388`).
- **Spellcheck:** uses Windows `ISpellChecker` (`crates/zgui-desktop/src/platform.rs:185-199 @a9dc282`).
- **Platform modules:** `popup_window/win32.rs`, `native_drag/win32.rs`, `window_place/win32.rs`, `native_host/{dcomp,webview2}.rs` (directory listing @a9dc282).

**Known Windows gaps** (`docs/windows.md:390-399 @a9dc282`):
- Cancelling a file picker or prompt doesn't dismiss the native dialog.
- No native Win32 menu bar; menus are rendered by zgui.
- Ghostty terminals and `NativeContent::Platform` are macOS-only.
- Menus and popovers are flat, not acrylic (`:158-163`).
- IME reconversion is not offered (`CHANGELOG.md:39`).

**Validation level on Windows:**
- The CI `windows` job runs clippy (with `webview2`), unit tests of `zgui`/`zgui-components`/`zgui-desktop`/`zgui-script` (`--lib --test components --test bundle`), an AccessKit heading test, and D3D12-on-WARP pixel tests. The workflow itself says "native Windows probes and the GPU suite run on developer machines" (`.github/workflows/ci.yml:262-289 @a9dc282`).
- Real-input smoke scripts exist (`scripts/windows_*_smoke.py`) but run by hand (`docs/windows.md:401-484`).
- `docs/ROADMAP.md:5 @a9dc282`: "Primary platforms: **Linux and macOS** … Windows remains a portability target". That file was last touched in the initial import (`92fd6a4`, 2026-09-26), while `docs/windows.md` was updated 2026-10-23 (`ca39a49`). **Inferred:** the roadmap is stale and Windows work has since been heavy, but formally Windows is still the least-gated platform.

#### Linux

Supported natively on Wayland and X11 (`docs/linux.md:1-6 @a9dc282`). Needs fontconfig, xkbcommon and D-Bus dev packages plus `pkg-config` (`README.md:64 @a9dc282`). X11/Wayland/IME/clipboard/DPI/AT-SPI probes run in CI (`ci.yml:143-149 @a9dc282`). Wayland gives no global window placement (`docs/platform-services.md:142`).

### 3. Luau engine and the Rust↔Luau bridge

#### Binding and VMs

- **Binding:** `mlua = { version = "0.12.2", features = ["luau"] }` (`crates/zgui-script/Cargo.toml:34 @a9dc282`), patched in `vendor/mlua` for borrowed table walks (`Cargo.toml:89-92 @a9dc282`).
- **One VM per script:** each script, the app's and each plugin's, gets its own VM (`docs/scripting.md:24 @a9dc282`).
  - Globals are read-only and `lua.sandbox(true)` is set (`crates/zgui-script/src/lib.rs:2079-2084 @a9dc282`).
  - Heap limit: 1 GiB for the app, 32 MiB for a plugin.
  - Time budget per call: 50 ms of CPU, 500 ms for load; ×10 in debug builds (`lib.rs:89-121 @a9dc282`).
- **Runtime shape:** the script runs once per load and returns a node tree that Rust lowers to retained views. Scripts are called again only for handlers, component mounts and function-valued properties. Templates (`"{count}"`) and animations run without script calls (`docs/scripting.md:3`; `lib.rs:1-9 @a9dc282`).

#### Full `ui.*` surface

From the type definitions `crates/zgui-script/types/zgui.d.luau.in:1913-2147 @a9dc282`:

- **Layout / containers:** `column row div overlay button grid scroll scroll_x split section layer separator`
- **Text:** `text span rich markdown number shortcut_text`
- **Media / paint:** `image svg canvas bitmap shader path linear`
- **Controls:** `input textarea checkbox slider progress segmented stepper select combobox command_palette calendar date_picker time_field tabs tree data_grid disclosure accordion`
- **Overlays:** `modal popover menu menu_item submenu toast dismiss_toast`
- **Lists / dynamic:** `list each show switch presence`
- **State:** `state shared persist use_state computed effect batch ref component on_mount on_cleanup object sort json.encode json.decode null now`
- **Actions / host:** `set toggle add action invoke call call_sync task fetch`
- **System:** `clipboard.read/write open_files save_file read_file write_file args on_open window command`
- **Native:** `web webview term terminal`
- **Timers / animation:** `every after tween spring loop`, plus the separate `anime` global (anime.js v4 port, `docs/scripting.md:416-456`)
- **Theme / design:** `theme.* prefers.* variants raw`
- **Plugins:** `slot contribute`
- **Element types:** `State`, `Promise` (`after`, `done/value/error`, `await`), `Element`, `Ref`, `Web`, `Term` (`zgui.d.luau.in:413-760`)

#### Data path

**Values.** `zgui_script::Value` is `Nil | Bool | Num(f64) | Str(Rc<str>) | List(Rc<[Value]>) | Map(Rc<[(Rc<str>, Value)]>) | Element` (`crates/zgui-script/src/value.rs:19-36 @a9dc282`).
- Values are immutable snapshots with structural sharing. Equality stops at shared `Rc`s (`value.rs:1-7, 61-80`).
- **`Value` is `Rc`-based, so it is `!Send`.** Values can only be built on the UI thread (**inferred** from types).
- JSON round-trip: `to_json`/`from_json` (`value.rs:109, 151`).

**Rust → Luau.** `ScriptHost::shared(key, initial) -> Shared` (`lib.rs:1192-1199 @a9dc282`).
- `Shared::set` **reconciles**: the new value keeps the old one's `Rc` parts where they are equal, and list items with an `id` are matched by it (`value.rs:156-162, 1624-1640`; `docs/reference/state.md:54`).
- "a model rebuilt and republished whole on every change … is the same value everywhere it did not change. Equal throughout, nothing is notified."
- `ui.each` then skips unchanged rows "at a pointer's glance". With the 3-parameter `build(item, i, row)`, a changed row updates in place (`docs/reference/dynamic.md:115-123 @a9dc282`).
- So updates are **fine-grained per row, but there is no patch/diff protocol**. Republishing a list is O(n) in Rust: rebuild the slice, then reconcile, which builds an id `HashMap` when lengths differ (`value.rs:184-225`). `Value::set_path` exists for path writes (`value.rs:307`), but lists are `Rc<[Value]>`, so an append still copies n pointers (**inferred**).
- Script side: `set_key`/`push`/`insert` copy only the changed path; `update` copies everything (`docs/reference/state.md:80`).

**Measured conversion costs:**
- 100,000 plain rows into `ui.state` takes about 60 ms; reading them out the first time takes about 110 ms (`docs/reference/state.md:137-139 @a9dc282`).
- `ui.call` costs about 1 µs on top of the host's work (`state.md:115`).
- `list_state_bench`: 5.0 M instructions per tick with NDEBUG (`docs/scripting.md:460`).
- Streaming 60 tokens/s, Luau vs Rust: CPU equal (2.6%), about 2% more instructions, +0.9 MiB memory (`docs/scripting.md:495`).

**Luau → Rust:**
- `ScriptHost::action` (fire-and-forget), `function` (sync, `ui.call_sync`), and `async_action` (`ui.call` → Promise) (`lib.rs:1084-1118 @a9dc282`).
- `async_action` futures "run on the window's tasks (the main thread, so it need not be `Send`) … hand blocking work to a thread and await its answer (`zgui::task::latest`)" (`lib.rs:1103-1108`).

#### Threading model

- Single UI thread. `Runtime`/`Signal` are `Rc` (`crates/zgui/src/reactive.rs:156-157, 487-488 @a9dc282`).
- `LocalExecutor` is single-threaded futures storage with a thread-safe wake queue: "Wakers may be used across threads" (`crates/zgui/src/task.rs:1, 92-121 @a9dc282`).
- "UI objects stay on the UI thread; use … channels for background work instead of moving `Rc`-backed state between threads" (`docs/application.md:182 @a9dc282`).
- `cx.tasks.spawn` is window-owned; `spawn_scoped` cancels on drop (`docs/scoped-tasks.md:1-29`).
- `scoped-tasks.md:29` says "use the background executor for blocking work", but **no public background executor exists**. The only match is a crate-private `file_dialog::background` (`crates/zgui-desktop/src/file_dialog.rs:472`, grep @a9dc282). You bring your own threads or tokio.

#### Pushing a chat event stream

- `zgui::task::latest::<T>()` is a cross-thread sender plus a UI-side receiver. It **keeps only the latest value: values sent before the task reads them coalesce** (`crates/zgui/src/task.rs:259-344 @a9dc282`; test at `:419-462`). That suits "latest snapshot" or "something changed", and it **drops intermediate messages** if you send events.
- The repo's working pattern for a socket is the `zeron` example: a background thread owns a `tungstenite` socket and pushes frames into an `Arc<Mutex<Inbox{Vec, Waker}>>`. The UI task drains it and wakes on push (`crates/zgui-desktop/examples/support/zeron_rpc.rs:8, 31-69, 71-149 @a9dc282`).
- For a Discord client, the path would be: tokio gateway task → your own queue + `Waker` → a UI-thread task that batches the events into your Rust model → `Shared::set` (or a `latest` "dirty" ping), under `runtime.batch`. **Inferred** design. Every part exists, but no off-the-shelf multi-producer event queue is provided.
- **tokio futures cannot run directly as `fetcher`/`async_action` futures.** Those are polled on zgui's `LocalExecutor`, not a tokio reactor. You must bridge via channels (**inferred** from executor design; `task.rs:94-191`).

#### Sandboxing and plugins

- Plugins get nothing unless granted: `Capability::{Fetch, Clipboard, Files, Terminals, Web, Windows, Named}` (`lib.rs:607-634 @a9dc282`).
- Host actions and shared keys must be `expose_action`/`expose_shared`'d (`lib.rs:1173-1191`).
- A plugin's files are always source, never bytecode (`docs/scripting.md:314-335 @a9dc282`).
- Bundled release builds compile scripts to bytecode via `build::bundle` + `include_bundle!` (`docs/scripting.md:362-378`).

#### Embedding Rust views inside a Luau tree

There is **no element kind that hosts an arbitrary Rust `View`**. The script node kinds are the fixed enum at `crates/zgui-script/src/node.rs:658-~890 @a9dc282`, with no host-view variant. **Inferred**, based on that enum and on the absence of any API in `lib.rs` `pub fn` list.

Rust can still place `host.view()` beside its own views, since `cx.render(host.view())` is a normal `View` (`docs/scripting.md:14-20`). A hybrid layout with a Rust transcript pane is therefore possible only at the Rust layout level (**inferred**).

### 4. Long lists, chat scrolling and anchoring

**`ui.each`** is keyed diffing/reconciliation:
- `key` keeps rows across reorders, inserts and removals. Without `key`, rows follow their value (`docs/reference/dynamic.md:27, 115-123 @a9dc282`).
- It builds **every** row and warns past 1,000 items (`docs/scripting.md:382 @a9dc282`).

**`ui.scroll { follow = true }`** "stays at the end while the content grows, until scrolled away" (`docs/reference/scrolling.md:19 @a9dc282`; lowered to `view.follow_end` at `crates/zgui-script/src/lower.rs:1032-1050`). It is **not virtualized**: "All of a `scroll`'s children stay mounted … past a few thousand rows, use `list`" (`scrolling.md:101`).

**`ui.list`** is virtualized: `count` with fixed `height`, or `heights` (app-supplied, "never measured"), plus `sticky` headers, `overscan`, `offset`, `keyboard` (`scrolling.md:27-45`; `zgui.d.luau.in:1715-1742 @a9dc282`). Limits:
- Rows are **keyed by index** ("a row's own state stays with its position, not its data", `scrolling.md:98`; lowered with `|index| index` at `lower.rs:3570, 3600`).
- There is **no `follow` option** on `ListProps` (`zgui.d.luau.in:1715-1742`).
- Variable-height rows need heights computed by the host: "A host can publish the heights as a shared value" (`scrolling.md:43`).

**Rust-only primitives:**
- `variable_virtual_list`, `measured_virtual_list` and `measured_rows` (keyed rows of natural height, heights kept by key, async estimates) (`crates/zgui/src/compose_variable.rs:110-163 @a9dc282`).
- `VariableHeights::anchor_end()`: "a chat transcript that follows new messages and stays put while offscreen estimates above it are measured" (`compose_variable.rs:34-41`).
- The `zeron` example uses exactly this for its transcript (`crates/zgui-desktop/examples/zeron.rs:375, 633 @a9dc282`).
- `docs/architecture.md:39-43 @a9dc282` documents the virtual-list family.

**Prepend anchoring (loading older history):**
- The variable list re-anchors on the first visible row only when `previous.length == length` (`compose_variable.rs:428-450 @a9dc282`). Doc: "Count changes preserve/clamp the pixel offset" (`compose_variable.rs:10`).
- Prepending N rows therefore shifts the visible content by their height (**inferred**). The app must add the inserted height to `offset` itself. That is feasible because `offset` is a public `Signal<f32>` and `VariableHeights::row_offset` is exposed (`compose_variable.rs:86-89`).
- No built-in "scroll anchoring" feature exists at either level. **Missing; build it in Rust.**

### 5. Images

**Formats:**
- Feature-gated SVG/PNG/JPEG/GIF (`docs/images-and-animation.md:39-52 @a9dc282`).
- `decode_image` handles PNG/JPEG via the `image` crate with an 8192² / 64 MiB cap (`crates/zgui-gpu/src/assets.rs:5-40 @a9dc282`).
- GIF goes through the separate `gif` crate (`crates/zgui-gpu/Cargo.toml:22, 50`; `decode_gif` at `assets.rs:132`).
- **No WebP or APNG**: grep finds only `WebProfile`. Missing.

**Luau path:**
- `ui.image { src }` takes a script-relative path, an `https://…` URL "through the host's fetcher", or `host:<name>`, "an image the app made" (`zgui.d.luau.in:1424-1433`; `docs/reference/media.md:15-25 @a9dc282`).
- Remote images: "one request per URL however many nodes show it, decoded off the main thread, cached for the host's life" (`docs/reference/state.md:121 @a9dc282`).
- Code: `std::thread::spawn` **per decode** + `latest` back to the UI (`crates/zgui-script/src/lower.rs:2643-2660 @a9dc282`). Images sit in a plain `HashMap` with no eviction (`lower.rs:28-51`). Failures retry with backoff.
- **Inferred:** with thousands of avatars, the cache grows without bound and decode threads spike. A Discord client should do its own fetch, decode and LRU in Rust, then publish via `ScriptHost::image(name, Some(Arc<ImageData>))` / `None` to evict (`lib.rs:1119-1125`).
- **GIF in Luau:** `load_image` calls `decode_image`, which handles stills only (`lower.rs:2694-2718`). zgui-gpu's `gif` feature enables `dep:gif`, not `image/gif`, so a GIF file in `ui.image` likely fails to decode (**inferred** from features at `crates/zgui-gpu/Cargo.toml:16-23`). No animated image in Luau.

**Rust path:**
- `ImageCache` is a bounded LRU: 16 MiB default (up to 64), 256 keys, shared in-flight loads, app-supplied transport (`docs/images-and-animation.md:3-14`; `crates/zgui/src/image_cache.rs:47-50, 105 @a9dc282`).
- `async_image` provides loading, error and success views (`crates/zgui/src/compose.rs:1443`).
- `animated_image` / `animated_image_controlled` handle GIF disposal and pause offscreen or when hidden (`docs/images-and-animation.md:16-35`; `compose.rs:1388-1400`).

**Inline images in text:**
- A span's `icon = { src | code, size, gap, tint }` draws "an icon before the text, in the line"; `src` may be `https://…` (`docs/reference/text.md:75`; `zgui.d.luau.in:1394-1404 @a9dc282`).
- A custom emoji can be an empty-text span with an icon (**inferred**). It is square-only, not animated, and there is no `host:` mention for icons (unverified).
- **Colour emoji glyphs:** the atlas copies `SwashContent::Color` glyphs (`crates/zgui-gpu/src/lib.rs:5574 @a9dc282`). Whether Windows' Segoe UI Emoji (COLR) renders in colour is **not verified in-repo**.

### 6. Text

**Composer:**
- `ui.input` / `ui.textarea` give selection, undo, clipboard and IME.
- Options: `multiline`, `wrap`, `placeholder`, `spellcheck` (system checker), `secure`, `on_change`/`on_commit`, highlight/decorations.
- Element API: `select_all`, `set_selection`, `replace_selection`.
- Long texts are laid out per line (`docs/reference/controls.md:40-69 @a9dc282`; `zgui.d.luau.in:1451-1501`).
- Enter-to-send needs an ancestor `on_key` returning `true`. `KeyEvent.editing` tells you a field has focus (`docs/reference/events.md:41, 219 @a9dc282`). Interaction with IME composition is **unverified**.
- No auto-grow-to-content option for `textarea` was found. Its default is `h = 156` (**inferred** from the options table).

**IME on Windows:**
- The Korean IME is smoke-tested with real keystrokes. Japanese and Chinese need an admin install to test (`docs/windows.md:428-435`).
- Known gap: no reconversion (`CHANGELOG.md:39`).

**Selectable read-only text:**
- `selectable = true` selects across nodes (word, paragraph, Shift, Ctrl+A/C), copies as text + HTML, and has a context menu (`docs/reference/text.md:158 @a9dc282`).
- "a selection in `ui.list`'s rows outlives the rows it recycles" (same line).

**Rich text:**
- `ui.rich` with spans (colour, bg, size, weight, bold/italic/underline/strike, mono font, `on_click`, `href`, `hover`, `padding`/`radius` chips, `icon`) (`text.md:50-75`).
- Span styles are read once and don't follow state (`text.md:165`).
- "This API is display text, not a rich-text editor" (`docs/rich-text.md:39 @a9dc282`).

**Markdown:**
- `ui.markdown(text, { key, breaks, paths })` parses CommonMark + tables/strikethrough/tasks (raw HTML dropped) into blocks for the script to draw. With `key`, it parses incrementally (`text.md:109-123 @a9dc282`).
- Rust: `zgui_script::markdown::{parse, Incremental}`.
- No Discord dialect (`||spoiler||`, `<@id>`, `<:emoji:id>`, `<t:…>`, `-#` subtext). Build it in Rust (**inferred**).

### 7. Networking and system services

- **HTTP:** `ui.fetch(url, { method, headers, body })` goes "through the host's transport, `ScriptHost::fetcher` … zgui adds no HTTP dependency … Without a fetcher, `ui.fetch` is an error" (`docs/reference/state.md:117-119 @a9dc282`; `lib.rs:1126-1138`).
  - `Request`/`Response` carry raw byte bodies (`crates/zgui-script/src/calls.rs:32-51 @a9dc282`).
  - `zgui-run` installs no fetcher (no `fetcher` in `crates/zgui-run/src`, **inferred** via grep).
  - The terminal example still uses `/usr/bin/curl` at HEAD (`crates/zgui-desktop/examples/terminal/plugins.rs:374-425 @a9dc282`; the example's own `host.fetcher` is at `examples/terminal.rs:248`).
- **WebSocket:** none in the library. `tungstenite` is only a **dev-dependency** for the `zeron` example (`crates/zgui-desktop/Cargo.toml:135 @a9dc282`). Missing.
- **Notifications (OS toast / Action Center):** none. Grep finds no `ToastNotification`, `UNUserNotification` or `org.freedesktop.Notifications`. `ui.toast` is an in-window component (`zgui.d.luau.in:2126`). Missing.
- **Tray icon:** none (grep `Shell_NotifyIcon`/`tray` @a9dc282). Missing.
- **Audio playback:** none. Grep hits only Ghostty bindings. Missing.
- **Clipboard:**
  - Luau: `ui.clipboard.read(): string?`, `write(text)` (`zgui.d.luau.in:2081-2085`).
  - Rust host: text, text+HTML, Linux primary selection (`crates/zgui-desktop/src/clipboard.rs:42-91 @a9dc282`; `crates/zgui/src/platform.rs:27-59`).
  - No image clipboard; `arboard` has default features off (`crates/zgui-desktop/Cargo.toml:52`). Partial.
- **File dialogs:**
  - Luau: `ui.open_files`/`save_file`/`read_file`/`write_file`, user-picked files only (`docs/reference/state.md:123-127`).
  - Rust: native owned dialogs on Windows (`docs/platform-services.md:1-9`).
  - Available. Note that `read_file` returns **text** (`Promise<string>`), so binary attachments need Rust (`state.md:34`).
- **OS file drop:** Rust `View::on_files_drop` (`docs/platform-services.md:147`); Windows OLE drops are tested (`docs/windows.md:436-440`). Luau `on_drop` takes zgui drag payloads only (`zgui.d.luau.in:1330`; no `files_drop` in `zgui-script`, grep). Partial.
- **Open URL:** `open_url` / span `href` (http, https, mailto) (`docs/platform-services.md:122`; `docs/rich-text.md:41`). Available.
- **Single instance / URL scheme / deep links:** `application_id` → `ApplicationEvent::OpenUrls`, Luau `ui.on_open` (`docs/platform-services.md:124-136`; `docs/reference/state.md:127`). Available.
- **Multiple windows:** `ui.window` (pop-out DMs) (`docs/scripting.md:22`; `zgui.d.luau.in` `WindowApi`). Available.
- **Persistence:** `ui.persist` writes JSON per key to `%APPDATA%\<app_id>\script-state`, batched 250 ms, written off-thread (`docs/scripting.md:303-312`). Available, but not suited to message caches (**inferred**: one JSON file per key).

### 8. Performance and memory

- **macOS heavy dashboard** vs GPUI (Apple Silicon, `dist` builds, medians, `README.md:66-81 @a9dc282`):

  | Mode | zgui | GPUI fast |
  |---|---|---|
  | Idle CPU | 0.00% | 0.38% |
  | Idle memory | 116 MB | 111 MB |
  | Stream CPU | 8.8% | 15.3% |
  | Stream memory | 259 MB | 259 MB |

  - zgui's p99 frame interval was 17.5–17.6 ms against 24.4–25.3 ms for GPUI.
  - A correction notes an earlier run accidentally used mimalloc (`README.md:79`).
  - "Windows and Linux are not measured against GPUI yet" (`README.md:81`).
- **Windows** (RTX 4090, 60 Hz, `docs/windows.md:67-105 @a9dc282`):
  - Gallery at rest: 2.7 M cycles/s.
  - Keystroke to composed screen: 12–14 ms median, 12 ms while animating.
  - Animation runs at every refresh with none dropped.
  - Resize latency is on par with or better than GDI.
  - mimalloc: 13% less tick time, about 40 MiB more committed memory (`docs/windows.md:32-49`).
  - **No idle RSS / working-set figure for Windows** in docs.
- **Linux (llvmpipe software GPU):** zgui mean RSS medians of 103–108 MiB in the 36-trial comparison (`docs/ROADMAP.md:54 @a9dc282`). Skipping GL init saves about 22 MiB idle RSS (`docs/validation.md:151`).
- **Engine costs:**
  - Idle flush is constant time; an idle window requests no frames (`docs/architecture.md:45-60 @a9dc282`).
  - `anime_bench`: 1,000 elements at about 90 µs per frame (`docs/scripting.md:470-478`).
  - Luau vs Rust streaming: +0.9 MiB, about the same CPU (`docs/scripting.md:495`).
- **Binary size:** about 12.1 MiB x64 for `component_workload` with fat LTO (`docs/windows.md:312-329`).
- **Tooling:**
  - `ZGUI_PROFILE=1` per-phase timing (`docs/benchmarking.md:32-42`).
  - CI instruction-count perf gate, failing above +1% (`ci.yml:160-201`).
  - `ScriptHost::script_calls()` lets tests hold hot paths at zero script calls (`docs/scripting.md:489`).

### 9. Fit verdict, detail

- **Use as-is:** windowing and DX12 rendering on Windows/Linux, retained fine-grained reactivity, Luau view layer with hot reload and sandboxing, text input/IME/selection, rich text, the markdown parser, file dialogs, deep links, multi-window, theming.
- **Build in Rust:**
  - Gateway WebSocket (+ zlib-stream) and REST client.
  - A multi-event UI queue (or reuse the `zeron_rpc.rs` Inbox pattern).
  - Image pipeline: WebP/GIF/APNG decode, bounded LRU, feeding `ScriptHost::image("host:…")`.
  - Chat transcript virtualization with measured heights, stick-to-bottom and prepend anchoring, as a Rust view next to the Luau UI or by computing heights in Rust for `ui.list { heights }` and managing `offset`.
  - Discord markdown dialect.
  - OS notifications, tray, voice/audio, image clipboard, file-drop exposure to Luau, binary attachment reads.
- **Watch:** the 9 required `[patch.crates-io]` forks; API churn at 0.1; Windows native behaviour validated by hand only.

---

### Sources (all `zeronsh/zgui` @ `a9dc28246170b4099a414191ca6fc172586b0f9c` unless noted)

**Docs:**
- `README.md`, `CHANGELOG.md`, `LICENSE`
- `docs/windows.md`, `docs/linux.md`, `docs/architecture.md`, `docs/scripting.md`, `docs/application.md`, `docs/scoped-tasks.md`
- `docs/platform-services.md`, `docs/images-and-animation.md`, `docs/rich-text.md`, `docs/benchmarking.md`, `docs/validation.md`
- `docs/ROADMAP.md` (last changed `92fd6a4`), `docs/gpui-parity.md` (last changed `92fd6a4`), `docs/releasing.md`, `docs/getting-started.md`
- `docs/reference/{index,state,scrolling,media,text,controls,dynamic,events}.md`

**Build / CI:**
- `Cargo.toml`, `.cargo/config.toml`, `.github/workflows/ci.yml`
- `crates/{zgui,zgui-gpu,zgui-desktop,zgui-script,zgui-run,zgui-components}/Cargo.toml`
- `comparisons/gpui/Cargo.toml`

**Code:**
- `crates/zgui/src/{task.rs, reactive.rs, compose_variable.rs, image_cache.rs, compose.rs, platform.rs}`
- `crates/zgui-script/src/{lib.rs, value.rs, lower.rs, node.rs, calls.rs}`
- `crates/zgui-script/types/zgui.d.luau.in`
- `crates/zgui-gpu/src/{assets.rs, lib.rs}`
- `crates/zgui-desktop/src/{clipboard.rs, platform.rs, file_dialog.rs}`
- `crates/zgui-desktop/examples/{zeron.rs, support/zeron_rpc.rs, terminal.rs, terminal/plugins.rs}`

**git / GitHub:**
- `git log` (activity, authors), `git rev-list --count c804b803..HEAD` = 287
- CI run for HEAD: https://github.com/zeronsh/zgui/actions/runs/40113007210 (success)
- `gh repo view zeronsh/zgui` (public, MIT, created 2026-09-26, pushed 2026-10-24)

**Reference copy:** `C:\Users\ADMIN\Desktop\nguyenvu\zgui-zterm-src\PIN.txt` → `c804b803587e802cf077b1133cc05309bc95139f`

---

# Part B: Discord user-account core

## Part: Discord user-account core (gateway, REST, auth, state)

Research for a lightweight native Discord client that logs in as a **user account**. The core is Rust; the UI is out of scope.
Date: 2026-10-25.

### Conventions

- **Concord citations** use `path:line` relative to the read-only clone `C:\Users\ADMIN\AppData\Local\Temp\concord-src\concord\` at HEAD `b312f18` (2026-10-24). Concord is `chojs23/concord` v2.6.3, GPL-3.0-only (`Cargo.toml:9`, `README.md:1198`). It was read only. It was never built or run.
- **[read]** means the fact was read directly in code or in a source document. **[inferred]** means it is my conclusion, not something the source states.
- **[official]** means discord.com or docs.discord.com, or discord.com/blog.
- **[community]** means community-maintained material: docs.discord.food (discord-userdoccers), discord.py-self, and similar. Treat it as reverse-engineered. It is not authoritative and it can go stale.
- Discord's official developer docs describe the **bot/OAuth2** API. Almost everything specific to user accounts (capabilities, client_state, op 37, READY_SUPPLEMENTAL, super-properties, login, remote auth) is **undocumented officially**. Those facts come from community docs or from concord.

---

### Summary and recommendations

#### Key protocol facts (short)

| Topic | Fact | Source |
|---|---|---|
| Gateway URL | `wss://gateway.discord.gg/?v=9&encoding=json&compress=zlib-stream` (concord). Official docs allow `compress=zlib-stream` or `zstd-stream`. | `src/discord/gateway.rs:121-125`; [official] gateway docs |
| API version | v9 and v10 are both "Available". Community docs say v9 is the client default. Concord uses v9 for REST and gateway. It uses v10 only for the QR ticket exchange. | [official] reference; [community] reference; `src/discord/qr_auth.rs:28` |
| Identify (user) | op 2 with `token` (raw, no `Bot ` prefix), `capabilities`, `properties` (super-properties), `presence`, `compress:false`, `client_state`. No `intents`. Concord sends capabilities = 253. | `src/discord/gateway.rs:2154-2218`, `:127-140` |
| Heartbeat | Use `heartbeat_interval` from HELLO. The first beat is jittered by interval × random[0,1]. A missing ACK means a zombie connection: close with a code other than 1000/1001, then RESUME. | [official] gateway; `src/discord/gateway.rs:1245-1277` |
| Send budget | 120 gateway events per 60 s per connection. Exceeding it disconnects immediately. | [official] gateway; `src/discord/gateway.rs:151-154` |
| Payload limits | Official: sent payloads ≤ 4096 bytes. Community docs and discord.py-self: 15 KiB, "taken from the client". Inbound user READY can exceed 16 MiB, so concord allows 64 MiB. | [official]; [community]; `src/discord/gateway.rs:142-145` |
| Member list | op 37 bulk guild subscriptions (`{"subscriptions":{guild_id:{typing,activities,threads,member_updates,members,channels:{ch:[[0,99],…]},thread_member_lists}}}`). op 14 is deprecated. The response is `GUILD_MEMBER_LIST_UPDATE` with SYNC/INSERT/UPDATE/DELETE/INVALIDATE ops. | `src/discord/gateway.rs:2285-2319`; [community] discord.py-self |
| REST | `https://discord.com/api/v9`. `Authorization: <token>` with no prefix. Browser-like headers plus `X-Super-Properties`, `X-Discord-Locale`, `X-Discord-Timezone`, and a per-channel `Referer`. A cookie jar is required. | `src/discord/fingerprint.rs:547-588`, `src/discord/rest.rs:116-137` |
| Rate limits | Learn buckets from `X-RateLimit-*` headers. Never hard-code them. On 429 read `retry_after`/`global`. Too many 401/403/429 responses (10,000 per 10 min) lead to a temporary IP ban. | [official] rate-limits; `src/discord/rest.rs:446-858` |
| Auth | Token paste (lowest risk). Email/password + TOTP/SMS via `/auth/login` + `/auth/mfa/{type}`. QR via `wss://remote-auth-gateway.discord.gg/?v=2` with RSA-2048 OAEP-SHA256. All of them can hit hCaptcha. | `src/discord/password_auth.rs`, `src/discord/qr_auth.rs`; [community] |
| ToS | ToS forbids "using any unauthorized software designed to modify the services". Discord Support: "Automating normal user accounts (generally called "self-bots") outside of the OAuth2/bot API is forbidden, and can result in an account termination if found." An unofficial *client* is not explicitly named, but it sits in a grey zone, and enforcement is automated. | [official] terms; [official] support |

#### Build-vs-reuse recommendation

1. **Do not vendor or translate concord code into an MIT repo.** Concord is GPL-3.0-only. Copying or porting its expression (even when rewritten line by line) makes the distributed combination subject to GPL. **Do reference it** as a behavioural spec. Protocol facts are not concord's expression: endpoints, field names, capability bits, header sets, timing constants, the shape of the reconnect state machine. **[inferred; not legal advice]**. Write our code from *this document plus community docs*, not side by side with concord's source. If a specific concord module is worth taking verbatim (e.g. `member/list.rs`, `fingerprint.rs`), ask the author for a written MIT/Apache relicence of those files.
2. **No existing crate is usable whole for a user account.**
   - twilight-http forces a `Bot ` prefix. twilight-gateway always sends `intents` and cannot send `capabilities` or `client_state`. twilight-model's `Ready` requires a bot-only `application` field.
   - serenity removed selfbot support in 0.2.0 (2017). Its HTTP client also forces a `Bot ` prefix.
   - Community self-bot crates (serenity_self, diself, discord-user-rs, hubbub[GPL]) are small, unvetted, and aimed at automation (self-bots).
3. **Reuse in part (permissive licences):**
   - twilight-model's `Id<Marker>` snowflake newtype (ISC). Concord itself adapts it from twilight, `src/discord/ids.rs:5-11`.
   - twilight-gateway's zlib/zstd `Decompressor` as reference or copied code (ISC, keep the notice). Note it is a private module in that crate.
   - Generic crates: `tokio-tungstenite`, `flate2` (zlib-rs) or `zstd-safe`, `reqwest` + `cookie_store`/`reqwest_cookie_store`, `serde_json`, `rsa` + `sha2` (QR), `keyring` 4 (`v1` feature → Windows Credential Manager, MIT/Apache).
4. **Architecture to build:**
   - `gateway`: a connection actor with session/resume state, a two-priority send queue behind a 120/60 s window, an op-8 scheduler, and op-37 debounce.
   - `rest`: bucket-learning limiter, mutation pacing, a 403 circuit breaker, stop-on-401, and captcha→stop.
   - `fingerprint`: live build number, super-properties, a persisted cookie jar and installation id.
   - `auth`.
   - `state`: Arc-per-area copy-on-write with revision counters, as concord does.
   - `media`: URL builders plus a bounded fetch/decode cache.
   - Lenient serde (`serde_json::Value` at the edges, typed structs with `#[serde(default)]`) so unknown user-only fields never break parsing.

#### Top risks

1. **Account action by Discord.** The ToS and the self-bot policy apply. Concord's FAQ lists concrete triggers: a new or low-reputation account, a first DM to an unknown user, a DM with little history, and captcha flows (`README.md:1164-1177`). Mitigation: token login by default, concord-style composer locks, no automation features, and conservative pacing.
2. **Fingerprint drift.** Concord compiles in Chrome `150.0.0.0` and a fallback build number of 580004, and scrapes the live build number from `discord.com/app` JS (`src/discord/fingerprint.rs:35-38`, `:659-680`). Discord can change any of this without notice. Inconsistent UA/super-properties/client hints are an anti-abuse signal [community reference: "highly recommended due to its significance in anti-abuse systems"].
3. **Captcha cannot be solved by a native client without embedding hCaptcha.** Concord stops and tells the user to finish in the official client. Retrying an unsolved captcha "is what escalates to a temporary account block (issue #218)" (`src/discord/rest.rs:944-948`).
4. **Undocumented protocol churn.** op 37, capabilities, READY shapes, and member-list ops come from reverse engineering. Plan for lenient parsing and a "raw dispatch" debug log, as concord keeps `GatewayDispatchReceived` (`src/discord/gateway.rs:1616-1626`).
5. **GPL contamination** if contributors copy from concord (see recommendation 1).
6. **Token = full account credential.** A token from concord's fallback file "can log in to the official Discord client" (`README.md:1183`). Store it in Credential Manager and never in plaintext.

---

### 1. Gateway

#### 1.1 URL, version, encoding, compression

- Concord connects to `wss://gateway.discord.gg/?v=9&encoding=json&compress=zlib-stream`. Its comment says it pins v9 "because the v9 dispatch shapes line up with everything `parse_user_account_event` already understands. Discord's browser client uses the stateful `zlib-stream` transport mode" (`src/discord/gateway.rs:121-125`) [read]. The claim that the *browser* client uses zlib-stream is concord's statement; I could not verify it independently [inferred/unverified].
- Official query params: `v`, `encoding` (`json`|`etf`), and optional `compress` (`zlib-stream`|`zstd-stream`). "Payload compression will be disabled if you use transport compression." [official] https://docs.discord.com/developers/events/gateway
- **zlib-stream**: buffer until the 4-byte `Z_SYNC_FLUSH` suffix `00 00 ff ff`. Use one shared zlib context per connection; "each Gateway connection should use its own unique zlib context" [official, same page]. Concord follows this:
  - one `flate2::Decompress` per connection, with a pending buffer that checks the suffix (`src/discord/gateway.rs:164-250`);
  - the decoder is created inside `connect_and_run` (`:1130`);
  - the same 64 MiB cap applies to compressed input and to decompressed output (`:189-225`).
- **zstd-stream**: one zstd context persists for the whole connection. Each WebSocket message is one gateway message but does not end a zstd frame, so you must keep calling the decompression function [official gateway page].
- **Why concord uses zlib, not zstd**: it impersonates the Chrome web client, and its stated reason is to match the browser (`gateway.rs:123-124`). The IDENTIFY sets `"compress": false` because "`zlib-stream` is selected in the Gateway URL. The browser keeps this separate Identify compression mode disabled" (`:2193-2195`) [read].
  - Discord's blog says zstd was rolled out to Android, desktop, and iOS ("roll out zstandard to all of our users on all platforms" over "a few months"). The post does not mention web [official blog] https://discord.com/blog/how-discord-reduced-websocket-traffic-by-40-percent.
  - **Recommendation [inferred]:** if we impersonate the web client (as concord does), use zlib-stream for consistency. zstd would only fit a "desktop client" persona, which needs a different super-properties set.
- **Encoding**: use JSON. ETF is binary and saves bandwidth, but it adds a dependency and is not what concord uses. Community docs say "JSON is generally recommended" [community] https://docs.discord.food/gateway/using-gateway.

#### 1.2 Payload limits

- **Sent:**
  - Official: "Must not exceed 4096 bytes". Exceeding it closes with `4002` [official gateway].
  - Community: "Must not exceed 15 KiB" [community using-gateway].
  - discord.py-self: `MAX_PAYLOAD_SIZE = 15360  # 15 KiB` with the comment "I thought it was 4096 bytes, but this is taken from the client" ([community] https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/state.py#L556-L557).
  - Plan for 4096 to be safe. Concord keeps op-37 range lists to at most 4 ranges because "Discord rejects oversized channel range lists" (`src/tui/state/subscriptions.rs:65-81`) [read].
- **Received:** "Some user-account READY payloads exceed tungstenite's default 16 MiB frame cap". Concord raises `max_message_size`/`max_frame_size` to 64 MiB (`src/discord/gateway.rs:142-145`, `:1515-1519`) [read].

#### 1.3 HELLO, heartbeat, ACK timeout

- Concord requires the first frame to be op 10 HELLO. Otherwise it fails fast (`gateway.rs:1132-1196`). The interval defaults to 41250 ms if absent (`:1197-1202`).
- First heartbeat after `rand(0..=interval)` jitter, then every `interval`. Payload `{"op":1,"d":<last seq or null>}` (`:1254-1277`).
- **ACK timeout:** a heartbeat is due while the previous one is still un-ACKed → treat as timeout and reconnect with **Resume** (`HeartbeatAckState`, `:822-839`; `:1259-1266`, `:1417-1419`).
- Server op 1 is answered immediately (`:1633-1644`). op 11 clears the pending flag (`:1662-1665`). WebSocket Pings get Pongs (`:1390-1397`).
- Official guidance matches: zombied connection → "terminate the connection with any close code besides `1000` or `1001`", then resume [official gateway].

#### 1.4 IDENTIFY for user accounts

Concord payload (`src/discord/gateway.rs:2154-2218`) [read]:

```json
{"op":2,"d":{
  "token":"<raw user token>",
  "capabilities":253,
  "properties":{"os":"Windows","browser":"Chrome","device":"","system_locale":"en-US",
    "browser_user_agent":"Mozilla/5.0 (Windows NT 10.0; Win64; x64) ... Chrome/150.0.0.0 Safari/537.36",
    "browser_version":"150.0.0.0","os_version":"10","referrer":"","referring_domain":"",
    "referrer_current":"","referring_domain_current":"","release_channel":"stable",
    "client_build_number":<live>,"client_event_source":null,"installation_id":"<if known>"},
  "presence":{"since":0,"activities":[],"status":"unknown","afk":false},
  "compress":false,
  "client_state":{"guild_versions":{},"highest_last_message_id":"<id or 0>",
    "read_state_version":<n or 0>,"user_guild_settings_version":<n or -1>,
    "user_settings_version":-1,"private_channels_version":"<id or 0>","api_code_version":0}}}
```

- **No `intents` field.** User accounts are filtered by subscriptions, not intents [inferred from concord payload + community GuildSubscriptions doc below].
- **Capabilities 253** = bits 0, 2, 3, 4, 5, 6, 7:
  - LAZY_USER_NOTES
  - VERSIONED_READ_STATES
  - VERSIONED_USER_GUILD_SETTINGS
  - DEDUPE_USER_OBJECTS
  - PRIORITIZED_READY_PAYLOAD
  - MULTIPLE_GUILD_EXPERIMENT_POPULATIONS
  - NON_CHANNEL_READ_STATES

  Concord's comment: "Without these bits set Discord assumes the session is a bot and silently drops friend presence streaming" (`gateway.rs:127-140`) [read; the causal claim is concord's].
- Full capability table, bits 0-16 [community] https://docs.discord.food/gateway/using-gateway:

  | Bit | Name |
  |---|---|
  | 0 | LAZY_USER_NOTES |
  | 1 | NO_AFFINE_USER_IDS |
  | 2 | VERSIONED_READ_STATES |
  | 3 | VERSIONED_USER_GUILD_SETTINGS |
  | 4 | DEDUPE_USER_OBJECTS |
  | 5 | PRIORITIZED_READY_PAYLOAD (requires 4) |
  | 6 | MULTIPLE_GUILD_EXPERIMENT_POPULATIONS |
  | 7 | NON_CHANNEL_READ_STATES |
  | 8 | AUTH_TOKEN_REFRESH |
  | 9 | USER_SETTINGS_PROTO |
  | 10 | CLIENT_STATE_V2 |
  | 11 | PASSIVE_GUILD_UPDATE |
  | 12 | AUTO_CALL_CONNECT |
  | 13 | DEBOUNCE_MESSAGE_REACTIONS |
  | 14 | PASSIVE_GUILD_UPDATE_V2 |
  | 15 | CHANNEL_OBFUSCATION |
  | 16 | AUTO_LOBBY_CONNECT |

  Concord deliberately does not enable USER_SETTINGS_PROTO. It reads legacy `user_settings` for guild folders and skips the protobuf (`src/discord/gateway/parser/ready.rs:198-204`).
- **client_state** fields [community] https://docs.discord.food/topics/gateway-events: `guild_versions`, `highest_last_message_id`, `read_state_version`, `user_guild_settings_version`, `user_settings_version`, `private_channels_version`, `api_code_version`, `initial_guild_id`. Concord: "Only reuse versions Concord actually tracks. The remaining conservative defaults avoid claiming cache state that this process cannot verify" (`gateway.rs:2184-2185`).
- **Presence on IDENTIFY:** status `unknown` on the first identify. On a re-identify, concord re-sends the last known status and activities (`gateway.rs:1222-1237`; `presence.rs:29-37`).
- **installation_id:** taken from `READY.apex_experiments.installation` and persisted (`gateway.rs:1582-1593`, `:1699-1704`). Community: installation IDs come from `/apex/experiments` (`X-Installation-ID`) and the Gateway `installation_id` [community] https://docs.discord.food/topics/experiments.

#### 1.5 RESUME vs re-IDENTIFY, INVALID_SESSION

- READY stores `session_id` and `resume_gateway_url` (`gateway.rs:1576-1611`). Resume needs all three of session_id, resume URL, and last seq (`:904-906`).
- The resume URL is sanitized: wss only, no credentials or fragment, and `v/encoding/compress` re-appended (`:980-1010`).
- RESUME: `{"op":6,"d":{"token","session_id","seq"}}` (`:2220-2230`). Official: reconnect to `resume_gateway_url` with the same version and encoding [official gateway].
- op 7 RECONNECT → Resume (`:1647-1650`).
- op 9 INVALID_SESSION: `d:true` → Resume, otherwise → Reidentify (`:1653-1661`). Official: `d:false` → reconnect and Identify [official gateway].
- **Failed resume handshake** (socket opens, then the handshake fails) → abandon the resume state and identify next time (`:1684-1697`, `:971-977`).
- A malformed JSON frame gets one Resume attempt per sequence, then Reidentify (`:952-964`, `:1502-1513`).
- On a re-identify, concord emits `GatewayReidentified`. That resets in-flight request bookkeeping and invalidates member lists (`request_lifecycle.rs:205-228`; `state.rs:1102`).

#### 1.6 Close codes that must not reconnect; backoff

- Concord mapping (`gateway.rs:1711-1728`) [read]:

  | Close codes | Concord outcome |
  |---|---|
  | 4004, 4010–4016 | **Fatal** (stop, show error) |
  | 4003, 4007, 4009 | Reidentify |
  | 4000–4002, 4005, 4008 | Resume |
  | other | Reidentify |
  | no close frame | Resume |

- Official table [official] https://docs.discord.com/developers/topics/opcodes-and-status-codes:
  - Reconnect = **false** for 4004 Authentication failed, 4010 Invalid shard, 4011 Sharding required, 4012 Invalid API version, 4013 Invalid intent(s), 4014 Disallowed intent(s).
  - The rest of 4000-4009 is true.
  - For a user client the critical one is **4004** (bad or revoked token) → go back to login.
- **Own shutdown:** closing with 1000/1001 invalidates the session [official]. Concord sends a normal close on deliberate shutdown, after an optional voice-leave (`gateway.rs:1292-1325`).
- **Backoff:** exponential with full jitter. Base 500 ms, doubling, max 30 s. It resets after any connection reached READY/RESUMED (`gateway.rs:149-150`, `:1065-1078`).

#### 1.7 Outbound pacing

- The concord writer task has two queues: urgent (heartbeat, identify/resume, voice) and normal. Both share one sliding window of 120 sends per 60 s (`gateway.rs:252-268`, `:1977-2135`).
- Official: "Apps can send 120 gateway events per connection every 60 seconds" and offenders "are immediately disconnected" [official gateway].

---

### 2. READY / READY_SUPPLEMENTAL

#### 2.1 What arrives (user account)

- **READY** (with DEDUPE_USER_OBJECTS + PRIORITIZED_READY_PAYLOAD). What concord parses (`src/discord/gateway/parser/ready.rs:29-211`) [read]:
  - `user`, plus verification flags `verified`/`phone`/`mfa_enabled` and premium tier
  - `guilds[]`: full guild objects with channels, threads, roles, emojis, voice_states. User accounts get no per-guild GUILD_CREATE stream, so concord synthesizes a `GuildCreate` per guild (`:29-32`, `:66-79`).
  - `merged_members[]`, parallel-indexed to `guilds[]` (`:313-334`)
  - `merged_presences`, `presences`
  - `users[]`: the deduplicated user directory. Private channels carry `recipient_ids` instead of recipients (`:92-123`, `:470-517`).
  - `private_channels[]`: partial when PRIORITIZED_READY_PAYLOAD is on (`state/caches.rs:161-163`)
  - `sessions[]`: own status from the `session_id=="all"` entry (`:423-440`)
  - `relationships[]` (`:150-163`)
  - `read_state`: with VERSIONED_READ_STATES this is `{entries, version, partial}` (`:165-180`)
  - `user_guild_settings`: versioned the same way (`:182-189`)
  - `notification_settings.flags`, `user_settings.guild_folders`
  - `session_id`, `resume_gateway_url`, `apex_experiments.installation` (`gateway.rs:1594-1601`)
- Community list of READY fields [community] https://docs.discord.food/topics/gateway-events:
  - core: `user, session_id, session_type, resume_gateway_url, auth_session_id_hash, auth_token, analytics_token, country_code, geo_ordered_rtc_regions`
  - settings: `user_settings(_proto), notification_settings, user_guild_settings, consents, tutorial, api_code_version`
  - guilds and members: `read_state, guilds, guild_join_requests, merged_members, merged_presences, presences, users`
  - social: `relationships, private_channels, connected_accounts, notes, sessions`
  - other: `experiments, guild_experiments, apex_experiments, required_action`, …
- **READY_SUPPLEMENTAL** (requires PRIORITIZED_READY_PAYLOAD) [community same page]:
  - `guilds` (supplemental objects: `id`, `voice_states`, `activity_instances`)
  - `merged_members`, `merged_presences`
  - `lazy_private_channels` ("DMs omitted from Ready because they were already cached")
  - `disclose`, `game_invites`

  Concord applies guild extras (onboarding, roles, channels, threads, members, presences, voice states), merged members and presences, and `lazy_private_channels` with `recipient_ids`. It then marks the supplemental phase complete (`ready.rs:229-261`, `:336-403`).

#### 2.2 Typical size

- Discord's own example: a READY of **2,517,725 bytes** uncompressed compressed to 306,745 bytes with zstd [official blog, link above].
- Concord notes some READYs exceed **16 MiB** (`gateway.rs:142-144`).
- **[inferred]:** expect 1-5 MB for typical accounts and tens of MB for accounts in hundreds of large guilds. Parse READY on a worker thread, not the UI thread.
- Concord parses each frame into `serde_json::Value` once and moves `d` out to avoid a second parse (`parser.rs:69-82`).

#### 2.3 Caching to shrink later READYs (`ClientCacheState`)

- `ClientCacheState { highest_guild_message_id, highest_private_message_id, read_state_version, user_guild_settings_version }` (`src/discord/state.rs:74-85`).
- The two message cursors are the max `last_message_id` over guild and private channels. Read acks are deliberately excluded (`state.rs:153-178`).
- Versions advance from READY, from `MESSAGE_ACK.version`, and from `USER_GUILD_SETTINGS_UPDATE` (`state.rs:1723-1809`).
- They are sent as `highest_last_message_id`, `private_channels_version`, `read_state_version`, and `user_guild_settings_version` (default −1). `guild_versions` is always `{}` and `user_settings_version` is −1 (`gateway.rs:2196-2214`).
- When the server answers with `partial:true`, concord merges instead of replacing (`state.rs:1729-1735`).
- **Limitation [inferred from code]:** `DiscordState` is not serialized to disk (it derives only `Clone, Debug`, `state.rs:60-61`). So `client_state` only helps **re-IDENTIFY within one process**. A cold start sends zeros.
- **Opportunity for us:** persist guild/channel/read-state snapshots plus versions to disk. That would let `guild_versions` and `CLIENT_STATE_V2` shrink cold-start READYs too. That is new work with no reference implementation in concord.

---

### 3. Lazy loading: guild subscriptions and member lists

#### 3.1 op 14 vs op 37

- discord.py-self: `GUILD_SUBSCRIBE = 14  # Deprecated`, `BULK_GUILD_SUBSCRIBE = 37` ([community] https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/gateway.py#L349-L357).
  - op 14 payload: `{guild_id, typing, threads, activities, members, channels, thread_member_lists}` (`gateway.py#L819-L852`).
  - op 37 payload: `{"subscriptions": {guild_id: {...same minus guild_id, plus member_updates}}}` (`gateway.py#L854-L862`; types `types/gateway.py#L688-L702`).
- docs.discord.food lists "Update Guild Subscriptions" as a send event with no documented structure [community].
- Behaviour notes from discord.py-self's `GuildSubscriptions` docstring [community] (`state.py#L510-L554`):
  - "automatically subscribed to all guilds with < 75k members on connect".
  - Unsubscribed guilds "will not receive non-stateful events (e.g. MESSAGE_CREATE…)"; they get PASSIVE_UPDATE_V2 instead.
  - `typing` subscription makes the guild "subscribed … indefinitely".
  - `members` and `thread_member_lists` are capped only by payload size.
  - Requests are debounced by 0.5 s (`TICK_TIMEOUT`).
- **Concord sends only op 37** (`gateway.rs:2285-2319`) [read]:

```json
{"op":37,"d":{"subscriptions":{"<guild_id>":{"typing":true,"activities":true,"threads":true,
 "member_updates":true,"members":[],"channels":{"<channel_id>":[[0,99]]},
 "thread_member_lists":["<thread_id>"]}}}}
```

#### 3.2 When concord sends what

- **Open a guild channel:** `SubscribeGuildChannel` → op 37 with `[[0,99]]` (`gateway.rs:1827-1840`). It is triggered on channel activation (`src/tui/state/channels.rs:1416-1427`) and re-sent after re-identify (`src/tui/runtime/effects.rs:179-183`). On both RESUMED and re-identify, a history "catch-up after" request is also queued for the selected channel (`:184-190`).
- **Member sidebar scroll:** `UpdateMemberListSubscription` with ranges (`gateway.rs:1841-1864`).
  - Ranges are 100-member buckets: always `[0,99]`, plus at most two buckets near the viewport end. Total ≤ 3-4 ranges (`src/tui/state/subscriptions.rs:53-81`).
  - The target channel is the active channel, or the first viewable non-voice, non-category, non-thread channel, so the sidebar fills before a channel is opened (`subscriptions.rs:15-32`, `:83-106`).
  - Debounced **100 ms** and deduplicated on (guild, channel, thread, bucket, refresh_generation) (`request_lifecycle.rs:809-854`, `:960-966`).
- **Open a DM:** **op 13** `{"channel_id": ...}`, once per channel per connection (`gateway.rs:2275-2283`, `:790-804`). Community docs name op 13 "Request Call Connect" ("Request a private channels's pre-existing call information"), not a message subscription [community gateway-events]. **[inferred]:** concord mirrors the web client here. It is not needed for receiving DM messages.
- **Thread participants:** add `thread_member_lists:[thread_id]`. The server replies with one authoritative `THREAD_MEMBER_LIST_UPDATE` (`subscriptions.rs:34-51`).

#### 3.3 GUILD_MEMBER_LIST_UPDATE ops

- Parsed in `src/discord/gateway/parser/members.rs:112-211`:
  - top level: `guild_id`, `id` (list id), `member_count`, `online_count` (derived from non-"offline" groups if absent), `groups[]`
  - `ops[]`: `SYNC{range,items}`, `INSERT{index,item}`, `UPDATE{index,item}`, `DELETE{index}`, `INVALIDATE{range}`
  - items are `{"group":{id,count}}` or `{"member":{…, presence}}`
- Applied in `src/discord/member/list.rs` [read]:
  - A **stable** snapshot stays visible while a **refreshing** snapshot fills until all requested ranges are SYNCed (`:31-48`, `:99-147`).
  - INSERT/DELETE shift indexes and synced ranges (`:326-394`).
  - INVALIDATE or unknown ops force a refresh by bumping `refresh_generation`, which re-sends op 37 (`:149-166`, `:203-209`).
  - Member data itself goes to the member entity cache, not the list (`:13-21`; `state.rs:699-735`).

#### 3.4 op 8 Request Guild Members

- Concord payloads (`gateway.rs:2232-2273`):
  - search: `{"guild_id":[id],"query","limit"(≤100),"presences":true,"nonce"}`
  - by id: `{"guild_id":[id],"user_ids"(≤100),"presences":false,"nonce"}`
- Scheduler (`gateway.rs:321-780`):
  - one in flight at a time; per-guild delay from `RATE_LIMITED` dispatches (`d.opcode==8`, `meta.guild_id`, `meta.nonce`, `retry_after`) (`:1554-1575`, `:2027-2054`)
  - a newer search replaces a pending search for the same guild
  - by-id requests are merged into 100-id chunks
  - `GUILD_MEMBERS_CHUNK.nonce` acknowledges a request; unanswered requests expire after 2 min (`:156`)
  - on reconnect, requests are re-queued
- Search debounce 250 ms; minimum query 2 chars (autocomplete) or 1 char (popup) (`request_lifecycle.rs:80-106`, `:903-941`). By-id "hydration" is deduplicated with a 30 s TTL (`:730-807`).
- Community limits [community] https://docs.discord.food/gateway/using-gateway and gateway-events:
  - "1 request per guild per session per 30 seconds"
  - users need MANAGE_ROLES, KICK_MEMBERS, or BAN_MEMBERS for the full list (empty query)
  - `user_ids` max 100; `nonce` max 32 bytes

---

### 4. Minimum event set for an MVP text client

Concord's dispatch table: `src/discord/gateway/parser.rs:93-181`. State application: `src/discord/state.rs:277-1110` [read]. MVP subset:

| Event | Concord effect on state |
|---|---|
| READY / READY_SUPPLEMENTAL | Seeds guilds, channels, threads, roles, emojis, members, users, DMs, relationships, read states, guild settings, own presence (see §2). |
| GUILD_CREATE / UPDATE / DELETE, GUILD_ROLE_*, GUILD_EMOJIS_UPDATE | Upsert or remove the guild and dependent caches (`state.rs:279-370`). |
| CHANNEL_CREATE / UPDATE / DELETE, THREAD_CREATE / UPDATE / DELETE, THREAD_LIST_SYNC | Upsert channels or threads; delete cascades to timelines, pins, read states, selection (`state.rs:382-394`, `:556`, `:1112-1131`). |
| CHANNEL_RECIPIENT_ADD / REMOVE | Group-DM membership (`:426-433`). |
| MESSAGE_CREATE | Clears that author's typing entry. Updates mention/notify counters. Builds MessageState (redacting the body if the channel is cold). Upserts into the timeline. Own messages mark the channel read locally (`state.rs:1430-1509`). |
| MESSAGE_UPDATE | Partial field merge; body retained only for warm channels or pinned messages (`:610-626`). |
| MESSAGE_DELETE / DELETE_BULK | Remove from timeline (`:689-698`). |
| MESSAGE_REACTION_ADD / REMOVE / REMOVE_ALL / REMOVE_EMOJI | Count and `me` bookkeeping per emoji (`:637-661`; `message/state.rs:1212`). |
| TYPING_START | Per (channel, user) timestamp with a 10 s TTL, pruned on the next event; may upsert member (`state.rs:8-11`, `:826-848`). |
| PRESENCE_UPDATE | Guild-scoped and global status/activities, plus DM recipient status (`:768-802`). |
| GUILD_MEMBER_ADD / UPDATE / REMOVE, GUILD_MEMBERS_CHUNK, GUILD_MEMBER_LIST_UPDATE | Member entity cache plus the list structure (`:699-767`). |
| MESSAGE_ACK, CHANNEL_PINS_ACK, CHANNEL_UNREAD_UPDATE, USER_NON_CHANNEL_ACK / GUILD_FEATURE_ACK | Read state and version (`:964-1028`). |
| USER_GUILD_SETTINGS_UPDATE | Mute and notification settings plus version (`:1037-1042`). |
| RELATIONSHIP_ADD / UPDATE / REMOVE, USER_UPDATE | Friends and identity (`:874-896`). |
| CHANNEL_PINS_UPDATE | Invalidate the pin cache (`:667-678`). |

You can defer voice (VOICE_STATE/SERVER_UPDATE, CALL_*, STREAM_*), interactions, and application commands.

---

### 5. REST

#### 5.1 Base URL and headers

- Base: `https://discord.com/api/v9`, hard-coded per call (e.g. `rest/messages.rs:78-81`). v9 is the client default and v10 is available [community reference; official reference].
- **Authorization:** raw token with no prefix [community] https://docs.discord.food/reference. Concord: `header(AUTHORIZATION, &self.token)` (`rest.rs:130-132`).
- **Header set** (`src/discord/fingerprint.rs:532-614`) [read]:
  - `User-Agent`: Chrome 150 UA
  - `Accept-Language`
  - Chrome client hints `Sec-CH-UA` / `-Mobile` / `-Platform`
  - `Accept: */*` and `Accept-Encoding: gzip, deflate, br, zstd`
  - `Referer`, `Priority: u=1, i`, `Sec-Fetch-Dest/Mode/Site`
  - `X-Discord-Locale: en-US`, `X-Discord-Timezone` (IANA, via `iana-time-zone`)
  - `X-Super-Properties` (base64 JSON), and `X-Installation-ID` when known
  - `Origin: https://discord.com` only on non-GET/HEAD (`rest.rs:280-288`)
  - `X-Fingerprint` only on unauthenticated login and experiments requests (`fingerprint.rs:590-596`; `auth_http.rs:43-52`)
- **Referer per channel:** `https://discord.com/channels/{guild}/{channel}`, or `/channels/@me/{channel}` for DMs. It follows the selected channel; otherwise it is `https://discord.com/channels/@me` (`fingerprint.rs:256-268`; `rest.rs:96-114`; `client.rs:249-259`).
- **Cookies:** a persistent cookie jar (`reqwest_cookie_store`), saved to a private file with a 250 ms debounce. The jar always forces `locale=en-US` for discord.com hosts (`fingerprint.rs:323-530`).
  - The bootstrap request to `discord.com/app` seeds the jar, and the same client is reused for auth and REST (`:196-240`).
  - The media client is separate and **unauthenticated**, so third-party proxy URLs never see session cookies (`src/app/media_adapters.rs:28-48`).
- Community: X-Super-Properties is "highly recommended due to its significance in anti-abuse systems". `browser_user_agent` should match `User-Agent` [community reference].

#### 5.2 Message history pagination

- `GET /channels/{id}/messages?limit=50[&before|after|around=id]` (`rest/messages.rs:192-274`; `src/app/history_commands.rs:12`).
- Official: `before/after/around` are mutually exclusive; `limit` is 1-100, default 50; results are newest→oldest [official] https://docs.discord.com/developers/resources/message.
- Concord deduplicates by cursor. An empty "older" page marks the channel exhausted (`request_lifecycle/primitives.rs:106-180`; `request_lifecycle.rs:668-694`).

#### 5.3 Send, reply, edit, delete

- **Send:** `POST /channels/{id}/messages` with `{"mobile_network_type":"unknown","content","nonce":"<snowflake>","enforce_nonce":true,"tts":false,"flags":0}` (`rest/messages.rs:474-515`).
  - The nonce is a time-based snowflake (`src/discord/commands.rs:31-39`), so a pending optimistic message is reconciled with MESSAGE_CREATE by nonce (`src/tui/state/pending_messages.rs:40-180`).
  - Official: nonce is an integer or string of up to 25 chars. With `enforce_nonce` a duplicate returns the existing message [official message].
- **Reply:** `message_reference:{message_id}`. To suppress the reply ping, send `allowed_mentions:{parse:["users","roles","everyone"],replied_user:false}`, with `parse` spelled out so in-content mentions still work (`rest/messages.rs:489-499`).
- **Edit:** `PATCH /channels/{c}/messages/{m}` with `{content}` or `{flags}` (`:153-174`, `:517-528`). **Delete:** `DELETE` the same path (`:176-190`).
- Per-channel send mutex, a slow-mode cooldown recorded after success, and the 429 `retry_after` recorded as a cooldown (`rest/messages.rs:108-151`; `rest.rs:897-942`).

#### 5.4 Reactions, typing, read state

- **Reactions:**
  - add: `PUT …/reactions/{emoji}/@me`
  - remove own: `DELETE …/reactions/{emoji}/0/@me` (the `0` is the reaction type segment, normal vs burst)
  - list: `GET …/reactions/{emoji}?limit=100&type=0[&after]`

  Sources: `rest/reactions.rs:25-91`. Official: emoji must be URL-encoded, custom emoji as `name:id`; READ_MESSAGE_HISTORY is needed, plus ADD_REACTIONS for a new emoji [official message].
- **Typing:** `POST /channels/{id}/typing`, fire-and-forget (`rest/messages.rs:59-85`). Concord sends it while composing at most every **8 s**, never while editing (`src/tui/state/composer/state.rs:70`, `:745-769`). Its comment: "A message that arrives with no prior typing looks automated" (`rest/messages.rs:59-60`).
- **Ack:**
  - single: `POST /channels/{c}/messages/{m}/ack` with `{"token":null,"last_viewed":<days since Discord epoch>,"flags"?}` (`rest/read_state.rs:15-35`, `:68-76`; last_viewed computed at `src/discord/read/state.rs:109`)
  - bulk: `POST /read-states/ack-bulk` with `{"read_states":[{"read_state_type":0,"channel_id","message_id"}]}` in chunks of 100 (`rest/read_state.rs:11-12`, `:37-95`)
  - Acks are applied **optimistically** to local state, then the REST call is **debounced 1 s** per channel, keeping the max message id (`src/app/read_state_commands.rs:25-34`; `request_lifecycle.rs:951-1019`).
  - Community: `last_viewed` = "number of days since the Discord epoch"; ack-bulk fires MESSAGE_ACK etc. [community] https://docs.discord.food/topics/read-state.

#### 5.5 Attachment upload flow

- **Concord uses classic multipart:** `payload_json` plus `files[n]`, and `attachments:[{id:n,filename}]` in JSON. Sizes are validated per file against the account/guild limit (`rest/messages.rs:500-571`; `message_policy.rs:47-72`). Official: the `id` must match `n`, max request 25 MiB [official message].
- **Web-client cloud flow** [community] https://docs.discord.food/topics/cloud-uploads:
  1. `POST /channels/{id}/attachments {files:[{filename,file_size,id}]}` → `{attachments:[{id,upload_url,upload_filename}]}`
  2. `PUT` the bytes to `upload_url` (Google Cloud, up to 500 MiB)
  3. send the message with `attachments:[{id,filename,uploaded_filename}]`
  4. `DELETE /attachments/{upload_filename}` for unused uploads
- **[inferred]:** the cloud flow is what the real web client does for large files. Multipart is simpler for an MVP and works per concord.

#### 5.6 Rate limits and concord's request lifecycle

- **Official** [official] https://docs.discord.com/developers/topics/rate-limits:
  - headers `X-RateLimit-Limit/Remaining/Reset/Reset-After/Bucket`, plus `-Global`/`-Scope` on 429
  - 429 body `{message, retry_after, global, code?}`
  - "rate limits should not be hard coded"
  - invalid-request limit "10,000 per 10 minutes" → temporary IP ban
- **Concord limiter** (`rest.rs:446-858`) [read]:
  - keys by route template (snowflakes after `channels|guilds|webhooks|streams` = major param; other numeric ids → `:id`; the reaction emoji → `:reaction`), then by learned `X-RateLimit-Bucket` + major param (`:753-800`, `:702-714`)
  - **probe** behaviour: until a response reports `remaining`, only one request per key is in flight (`:575-581`)
  - reset from `retry-after` → `x-ratelimit-reset-after` → `x-ratelimit-reset` (`:833-848`)
  - a global 429 blocks everything
  - mutations with no rate-limit headers are throttled to 500 ms (`:640-652`)
- **Pacing** independent of buckets: all non-GET mutations ≥ **500 ms** apart, message sends ≥ **100 ms** apart (`rest.rs:72-74`, `:419-438`, `:152-167`).
- **Safety:**
  - any **401** stops all further REST (`AuthenticationStopped`) (`:316-357`)
  - **3 consecutive 403s** on a route open a 5-minute circuit (`:64-71`, `:353-393`)
  - **captcha 400** → `CaptchaRequired`, never retried (`:944-1016`; `captcha.rs:1-43`)
- **Request lifecycle** (`src/discord/request_lifecycle.rs`, `request_lifecycle/primitives.rs`): per-feature deduplication of in-flight work.
  - `OnDemandRequests`: a failed key retries only when the selection moves away and back.
  - `CursorRequests`: one cursor in flight; exhausted cursors are never re-requested.
  - `TimedRequestSet`: TTL plus capacity.
  - Completion is driven by `record_event` on published AppEvents (`:204-219`, `lifecycle.rs:21-54`).
  - This is the main protection against UI-driven request storms. **Worth re-implementing [inferred].**

---

### 6. Auth

#### 6.1 Token login

- Paste a token, then `GET /api/v9/users/@me`. A 401/403 → `DiscordTokenRejected` (`src/discord/rest/connection.rs:8-23`). The token must also be a valid header value (`client.rs:1167-1171`).
- Concord's README: "I strongly recommend using token authentication" because email and QR "may trigger a CAPTCHA challenge" (`README.md:183`).
- An env override is available (`CONCORD_TOKEN`) (`token_store.rs:19-25`).

#### 6.2 Email/password + MFA

- `POST /api/v9/auth/login {login, password}`. Phone numbers are normalized by stripping spaces and dashes (`password_auth.rs:134-162`, `:328-335`).
- Login headers add `Referer: https://discord.com/login`, `Origin`, `Cache-Control/Pragma: no-cache`, and `X-Fingerprint` (`auth_http.rs:8`, `:43-52`).
- Response handling (`:231-282`):
  - `suspended_user_token` → suspended
  - `required_actions` → surface them
  - `token` → done
  - `mfa:true` + `ticket` + `login_instance_id` + `totp`/`sms` flags → MFA
- MFA:
  - verify: `POST /auth/mfa/{totp|sms} {code, login_instance_id, ticket}` (`:197-229`)
  - SMS send: `POST /auth/mfa/sms/send {ticket}` → `{phone}` (`:164-195`)
  - Community also lists `backup`, `webauthn`, `password` authenticators [community] https://docs.discord.food/authentication.
- Errors (`:297-326`):
  - `captcha_key` → abort with "Log in with a token instead"
  - 50035 → bad credentials
  - 20011 / 20013 → disabled
  - 70007 / 70009 → phone or email verification required

#### 6.3 QR remote auth

Concord flow (`src/discord/qr_auth.rs:108-399`) [read]; community spec agrees [community] https://docs.discord.food/remote-authentication/desktop:

1. Connect to `wss://remote-auth-gateway.discord.gg/?v=2` with browser headers including `Origin: https://discord.com`. Community: Origin must be a discord.com host.
2. Read `hello{heartbeat_interval}` and generate RSA-2048.
3. Send `init{encoded_public_key = base64(SPKI DER)}`.
4. Receive `nonce_proof{encrypted_nonce}`, decrypt with RSA-OAEP-SHA256, send `nonce_proof{nonce = base64url(no pad)}`.
5. Receive `pending_remote_init{fingerprint}` and verify `fingerprint == base64url(sha256(SPKI))`.
6. Show the QR for `https://discord.com/ra/{fingerprint}`.
7. Receive `pending_ticket{encrypted_user_payload}`, which decrypts to `id:discriminator:avatar:username`.
8. Receive `pending_login{ticket}` → `POST https://discord.com/api/v10/users/@me/remote-auth/login {ticket}` with `X-Fingerprint` → `{encrypted_token}` → decrypt.

- Heartbeat `{"op":"heartbeat"}` / `heartbeat_ack`. Close 4003 = session timed out; concord reconnects up to 3 times (`:27-30`, `:287-299`).
- A captcha on the ticket exchange aborts (`:413-419`).

#### 6.4 hCaptcha

- Shape [community] https://docs.discord.food/topics/captcha-handling:
  - HTTP 400 with `captcha_key[]`, `captcha_service` (`hcaptcha`), `captcha_sitekey` (dynamic), `captcha_session_id?`, `captcha_rqdata?`, `captcha_rqtoken?`
  - retry with `X-Captcha-Key`, `X-Captcha-Session-Id`, and `X-Captcha-Rqtoken` headers
  - It applies to "logging in, accepting invites, and sending friend requests" and to suspicious bursts.
  - rqdata "must be used in the challenge request or the challenge will fail".
- Concord's detection: 400 + `captcha_key` contains `captcha-required` + service hcaptcha (or absent) (`captcha.rs:19-43`). It never solves or retries. Its comment says retrying "is what escalates to a temporary account block (issue #218)" (`rest.rs:944-948`).
- **What a native client can do [inferred]:**
  - (a) stop and send the user to the official client (concord's approach);
  - (b) embed a webview loaded from an hcaptcha.com page with the given sitekey/rqdata, hand the solution back, and replay the request with the headers above. This adds a webview dependency, and it is fragile because hCaptcha Enterprise scores the environment;
  - never use third-party solving services. That would be automation and is an obvious ToS and abuse signal.

#### 6.5 Token storage

- **keyring 4.2.0** (MIT OR Apache-2.0). The `v1` feature (default) selects the platform store automatically: "On Windows, the secure credential store is the Windows Credential Manager" (keyring-4.2.0 `src/v1.rs:1-13`, `Cargo.toml [features] v1`, downloaded from crates.io) [read]. `Entry::new(service, user)`, `set_password`, `get_password`, `delete_credential`.
- Windows limit: `CredentialBlobSize` ≤ `CRED_MAX_CREDENTIAL_BLOB_SIZE (5*512)` = 2560 bytes. That is ample for a token [official MS] https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw.
- Concord: service `io.github.chojs23.concord.discord-token.v1`, account `account:<id>` (`token_store.rs:8-11`, `:169-196`). Store modes are `auto` (keychain, then **plaintext TOML** fallback), `keychain`, `plain` (`:27-62`, `:198-261`).
  - On Unix the fallback file is 0600 inside a 0700 dir (`README.md:1181-1184`).
  - **Recommendation [inferred]:** on Windows, do not fall back to plaintext. If Credential Manager fails, use DPAPI (`CryptProtectData`) per user, or refuse to persist.

---

### 7. Fingerprinting and account safety

#### 7.1 What concord does and why

- **Live build number.** It GETs `https://discord.com/app`, finds the `/assets/sentry….js` path, fetches it, and extracts `buildNumber","<digits>` (5 s timeouts). The compiled fallback is `580_004` (`fingerprint.rs:35-36`, `:659-680`, `:757-774`; CHANGELOG `CHANGELOG.md:692`). Community: `client_build_number` is "used to gate experimental features and may be required for certain endpoints" [community reference].
- **Chrome persona.** `browser:"Chrome"`, `browser_version:"150.0.0.0"`, and UA `Mozilla/5.0 (<platform>) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36`. The platform is `Windows NT 10.0; Win64; x64` on Windows, and `os_version` is forced to "10" on Windows (Chrome reports that) (`fingerprint.rs:37-38`, `:776-795`).
- **Super-properties** (`fingerprint.rs:132-153`, `:633-657`): `os, browser, device:"", system_locale, browser_user_agent, browser_version, os_version, referrer:"", referring_domain:"", referrer_current:"", referring_domain_current:"", release_channel:"stable", client_build_number, client_event_source:null, has_client_mods:false, client_launch_id (uuid per launch), launch_signature, client_app_state:"focused", client_heartbeat_session_id (uuid)`. Gateway IDENTIFY `properties` is a subset plus `installation_id` (`gateway.rs:2160-2178`).
- **Launch signature.** A random UUIDv4 with fixed bit masks cleared (`fingerprint.rs:844-868`). Community: "certain bits are used to encode information about the client, specifically whether certain client mods are detected" [community reference]. **[inferred]:** the mask clears exactly the "mods detected" bits, so the client reads as unmodified.
- **Session identifiers.** Run in parallel at startup:
  - `GET /api/v9/experiments?with_guild_experiments=true` → `fingerprint` (sent as `X-Fingerprint` before login)
  - `GET /api/v9/apex/experiments?surface=2` → `installation` (`X-Installation-ID`)
  - Both are persisted (`fingerprint.rs:43-45`, `:682-748`).
  - Community: fingerprints are generated only without auth or fingerprint headers, with a limit of 3 valid per 2 min per IP [community] https://docs.discord.food/topics/experiments.
- **Cookies** (persisted jar, forced `locale`), **Referer per channel**, and **client hints** (see §5.1). Concord shares one fingerprint across gateway, REST, and auth (`CHANGELOG.md:625`; `client.rs:130-139`).
- **Behavioural mimicry:** typing before sends, debounced acks, mutation pacing, member-request scheduling, one history request per selection (sections above).

#### 7.2 Known block triggers and concord's guards

- **README FAQ** (`README.md:1164-1177`) [read], verbatim:
  - "If your account is new or has a low reputation, some actions may trigger a block."
  - "Trying to **create a new DM channel and send a message to an unknown user**… can immediately block your account temporarily."
  - "Sending a message in an existing one-to-one DM may also trigger a temporary account block if you only recently started messaging that person or have little conversation history with them."
  - "Some features that requires a hCapcha challenge."

  Concord adds: "Using unofficial clients, automated user accounts, or self-bots can violate Discord's TOS, so there is always some risk." CHANGELOG: "Block send message dm to fresh user (#222)" (`CHANGELOG.md:798`).
- **Composer safety locks** (`[safety] enabled = true` by default, `README.md:433-439`; `src/tui/state/composer/state.rs:423-502`):
  - **DM:** locked if `is_spam`, `is_message_request`, an empty channel (no `last_message_id` and nothing cached), or fewer than **5** cached messages from the current user and not otherwise established (`src/tui/state.rs:65`).
  - **Guild:** locked by verification and participation restrictions, slow mode, or an empty channel with no message count.
  - Always enforced: history must be loaded before sending, and history-load failure locks the composer.
- **Guild participation gate** (`src/discord/verification.rs:168-324`): owner bypass, pending membership screening, onboarding incomplete, verification level, roles, phone/email verified, account age, member age. It mirrors Discord's own verification levels so we don't send messages Discord would reject.
- **Action policy:** permission checks before every mutation (`src/discord/action_policy.rs`, `client/rest_actions.rs:97-921`). For example, reactions need READ_MESSAGE_HISTORY and ADD_REACTIONS for a new emoji. This avoids 403 storms, which also count toward the invalid-request limit.
- **Message policy:** non-empty content, length limit, attachment count and size (`message_policy.rs`).

#### 7.3 What Discord's terms say (verbatim)

- **Terms of Service** (Effective: September 29, 2025) [official] https://discord.com/terms:
  - "You may not copy, modify, create derivative works based upon, distribute, sell, lease, or sublicense any of our software or services. You also may not reverse engineer or decompile our software or services, attempt to do so, or assist anyone in doing so, unless you have our written consent or applicable law permits it."
  - Prohibited: "…scraping our services without our written consent, including by using any robot, spider, crawler, scraper, or other automatic device, process, or software; … using any unauthorized software designed to modify the services; … copying, dismantling, or reverse engineering any of our services…"
  - Also prohibited: "…spamming, auto-messaging, or auto-dialing people through our services…"
- **Support, "Automated User Accounts (Self-Bots)"** [official] https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots: "Automating normal user accounts (generally called "self-bots") outside of the OAuth2/bot API is forbidden, and can result in an account termination if found."
- **Support, "Protecting Your Data"** [official] https://support.discord.com/hc/en-us/articles/360039598252: "Self-bots are and have always been a violation of our API Terms of Use".
- **Developer Policy** (Effective July 8, 2024) [official] https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy:
  - "Do not modify a Discord user's account without explicit permission from the Discord user."
  - "Do not enable your Application to bypass or circumvent Discord's privacy, safety, and/or security features."
  - "Do not mine or scrape any data…"
- **[inferred]:** none of these texts names "third-party clients" explicitly. A manual, human-driven client is not automation, but "unauthorized software designed to modify the services" and the reverse-engineering clause can be read to cover it. Enforcement is automated (anti-abuse). Treat account loss as a real, user-borne risk and say so in the UI. Do not ship automation features (auto-reply, bulk actions, scraping, multi-account farming).

---

### 8. State and cache design

- **Arc-per-area copy-on-write.**
  - `DiscordState` holds `Arc<NavigationIndex>`, `Arc<MessageCache>`, `Arc<GuildDetailCache>`, `Arc<ProfileCache>`, `Arc<PresenceCache>`, `Arc<VoiceStateCache>`, `Arc<SessionState>`, `Arc<NotificationCache>`, `Arc<ThreadCache>` (`state.rs:57-72`; areas in `state/caches.rs:25-176`).
  - Writes go through `Arc::make_mut`, which copies an area only if a snapshot still references it (`state.rs:117-151`).
  - `snapshot()` is just refcount bumps (`:180-200`).
- **Revisions.**
  - `SnapshotRevision {global, navigation, message, detail}`. `advance(areas)` stamps the changed areas with the new global value (`state/snapshot.rs:6-73`).
  - Each `AppEvent` declares the areas it touches. The publisher applies it under a write lock, bumps the revision, and sends it on a `tokio::watch`. Events with side effects also go to a bounded mpsc of 4096 (`client.rs:1105-1165`, `:149-150`; `lifecycle.rs:21-34`).
  - MESSAGE_CREATE only marks `detail` dirty if a read-state signature actually changed (`state.rs:237-275`).
  - The UI reads lazily and re-renders only areas whose revision moved.
- **Message limits.**
  - `DEFAULT_MAX_MESSAGES_PER_CHANNEL = 200`; `DEFAULT_MAX_WARM_MESSAGE_CHANNELS = 10`; older-history extra window ×2 while scrolling (`state.rs:40-45`).
  - When a warm channel is evicted, its message **bodies are redacted** but metadata stays, and the channel becomes "cold" (`message/state.rs:879-917`).
  - Live messages in cold channels keep their bodies only if authored by us or mentioning us (`message/state.rs:767-809`).
- **Other caps** (`state.rs:46-51`): 256 profiles, 256 notes, member lists for the 10 most recent guilds. Typing TTL 10 s.
- **Reported memory.** "In observed typical use, it usually uses about 20-40 MB of memory". Image-heavy screens can "briefly rise to around 100-200 MB while decoding" (`README.md:1154-1162`) [read; self-reported, not measured here].
- **Worth borrowing [inferred]:**
  - the area split plus revisions (a perfect fit for a retained-mode GUI that diffs by area)
  - warm/cold body redaction
  - the event-driven request lifecycle
  - the stable vs refreshing member list
  - persisted versions
- **Change:**
  - persist the snapshot to disk for cold-start `client_state`
  - add an LRU for timelines beyond 10 warm channels if memory matters
  - use typed structs instead of `Value` for hot events

---

### 9. Media

#### 9.1 CDN URL formats

Official [official] https://docs.discord.com/developers/reference#image-formatting:

- Base `https://cdn.discordapp.com/`.
- `?size=` must be a power of two from 16 to 4096.
- `a_` hashes are animated. Request `.webp?animated=true` ("do not convert cleanly to GIF").

| Asset | Path |
|---|---|
| Emoji | `emojis/{id}.png` (WebP recommended) |
| Guild icon | `icons/{guild}/{hash}.png` |
| Avatar | `avatars/{user}/{hash}.png` |
| Default avatar | `embed/avatars/{index}.png`, index = `(user_id>>22)%6`, or `discriminator%5` for legacy users |
| Member avatar | `guilds/{g}/users/{u}/avatars/{hash}.png` |
| Sticker | `stickers/{id}.png|json|gif` (size ignored; GIF stickers via `media.discordapp.net/stickers/{id}.gif`) |

- Attachment URLs are **signed and expire** (`ex`, `is`, `hm`).

Concord:

- Avatars use `.gif` for `a_` hashes, otherwise `.png`, then add `size=` clamped 16-1024 from the cell size (`src/discord/avatar.rs:8-48`; `src/tui/media/protocol.rs:17-20`, `:79-80`, `:130-137`).
- Emoji: `emojis/{id}.webp?animated=true` or `.png`. Unicode emoji come from pinned Twemoji 17.0.3 on jsDelivr (CC-BY 4.0, attribution in README) (`src/discord/emoji.rs:3-43`).
- Stickers: `media.discordapp.net/stickers/{id}.{ext}?size=160&passthrough=…` (`src/discord/message.rs:15-16`, `:249`).

#### 9.2 Media-proxy resize

- Community [community] https://docs.discord.food/reference:
  - `media.discordapp.net` mirrors CDN paths and accepts `width`, `height`, `format`, plus CDN params `quality` (lossless|high|low), `animated` (WebP), `passthrough`, `keep_aspect_ratio`
  - unsupported media → 415
  - signed attachment params remain required
- Concord's preview URL: replaces `format/quality/width/height/animated` on the `proxy_url` with `format=webp[&animated=true][&quality=low|lossless]&width=W&height=H`. Dimensions are scaled from source dimensions (efficient 0.3, balanced 0.5, high 1.0) and capped at 1000 px (`src/tui/media/targets.rs:29-36`, `:595-679`).

#### 9.3 Fetch, decode, cache (concord)

- **Fetch.** One pooled, **unauthenticated** `reqwest::Client` (connect timeout 10 s, 6 idle conns per host). Its comment says creating a client per image made the CDN refuse connections. Previews are capped at 16 MiB, streamed in chunks; downloads use a 30 s idle timeout (`src/app/media_adapters.rs:19-84`).
- **Decode.** `image` crate plus `thorvg` for Lottie stickers (`Cargo.toml:34`, `:103-106`). Limits (`src/tui/media/decode.rs:26-42`):
  - 4096×4096 max, 64 MiB per decoded image
  - shared cache of 32 images / 128 MiB
  - ≤32 retained animation frames; 256 MiB animation work budget
  - 1 MiB Lottie JSON; frame delays clamped 50 ms–10 s
- **Caches:** preview 16 entries / 64 MiB (`preview.rs:24-26`); avatars 32 / 12 MiB (`tui/media/avatar.rs:26-27`); emoji 128 / 24 MiB (`tui/media/emoji.rs:25-26`). Two concurrent decode workers, 32 queued jobs (`work.rs:8-9`). Retry backoff on fetch failure (`cache.rs:431`).
- **[inferred]** for a GPU GUI:
  - keep the byte-budgeted LRU idea, but cache GPU textures keyed by (url, size)
  - request proxy sizes that match device pixels
  - refresh expired signed attachment URLs by re-fetching the message rather than retrying the stale URL

---

### 10. Build vs reuse

#### 10.1 How separable concord's `discord/` module is

- `src/discord/` has no imports from `crate::tui`. A grep for `crate::tui` under `src/discord` returns nothing [read].
- Its external couplings: `crate::logging`, `crate::{AppError, Result}`, `crate::config::SafetyOptions` (one call, `client.rs:208`), `crate::paths`, `crate::support::{tls, private_file, audio_output, media_player}` (the last two only for voice).
- A few behaviours live **in the TUI/app layer, not in `discord/`**:
  - op-37 range computation (`tui/state/subscriptions.rs`)
  - composer DM/empty-channel safety locks (`tui/state/composer/state.rs:423-502`)
  - the typing cadence (`composer/state.rs:745-769`)
  - the history page size (`app/history_commands.rs:12`)
  - media URL sizing, fetch, and decode (`tui/media/*`, `app/media_adapters.rs`)

  Any reuse or port must pull those out [read + inferred].
- Size of `src/discord/`: 71,336 non-test LOC (22,089 test LOC). **Voice is 32,465** of that (plus 3,362 tests) and RPC is 2,054. Text-relevant code is about 37k [read via `wc`; counts include inline `#[cfg(test)]` blocks in non-test files].

#### 10.2 Licensing

- Concord: **GPL-3.0-only**.
- **Copying**: verbatim, translated, or close paraphrase of any concord file. This creates a derivative work. Distributing it requires the whole combined program under GPL-3.0, with source. That is incompatible with keeping our repo MIT [inferred; standard GPL reading, not legal advice].
- **Referencing**: learning protocol facts (endpoints, JSON shapes, header names, constants, algorithms such as "debounce acks 1 s") and reimplementing them independently. That does not import copyright, because facts and interoperability information are generally not protected expression [inferred, not legal advice].
- **Practical rules:**
  - implement from this spec and the community docs
  - don't keep concord open while writing the equivalent module
  - cite concord in docs as a reference
  - keep the git history clean of pasted code
- Concord's `ids.rs` is itself adapted from twilight (ISC) (`src/discord/ids.rs:5-11`). Take that type from **twilight** directly, not from concord.
- discord.py-self is MIT (`LICENSE` "The MIT License (MIT) Copyright (c) 2015-present Rapptz"), so it is safe to read and even port. But it is Python and aimed at self-bots.

#### 10.3 Crates

| Crate | Licence | User tokens? | Usable part |
|---|---|---|---|
| twilight-http 0.17.1 | ISC | **No**: the builder prepends `Bot ` unless `Bot `/`Bearer ` is present (`twilight-http/src/client/builder.rs:138-144`); UA `DiscordBot (` (`client/mod.rs:141-142`) | Route and ratelimit ideas only |
| twilight-gateway 0.17.1 | ISC | **No**: Identify always carries intents; `IdentifyProperties` is bot-shaped; no `capabilities`/`client_state`; API v10; the compression module is private (`twilight-gateway/src/shard.rs:56-60`, `:567`, `:693-696`; `lib.rs:21`) | Decompressor code (copy under ISC); queue and heartbeat patterns |
| twilight-model 0.17.1 | ISC | **Partial**: `Ready` requires a bot-only `application` field (`twilight-model/src/gateway/payload/incoming/ready.rs:6-17`), so a user READY fails to deserialize [inferred from struct + community READY field list: "OAuth2 only: application"] | `Id<Marker>`; some leaf models (Message, Embed, Attachment) are probably usable with care [inferred] |
| serenity 0.12.5 | ISC | **No**: "Selfbot support has been completely removed" in 0.2.0 (`CHANGELOG.md:5000`); `parse_token` adds `Bot ` (`src/http/client.rs:169-175`) | none |
| serenity_self 0.13.3 (fork, repo now `seradotflac/serenity-self`, 33★, last push 2025-07-17) | ISC | Yes (claims) | Unvetted; automation-oriented; heavyweight |
| diself 0.3.0 (MIT), discord-user-rs 0.7.0 (MIT on crates.io, no repo licence file), discord-selfbot 0.1.0 (MIT), hubbub 0.10.1 (**GPL-3.0**, archived) | — | Yes (self-bot libs) | Unvetted, tiny user bases; not recommended as a foundation |
| keyring 4.2.0 | MIT/Apache-2.0 | n/a | Token storage (Windows Credential Manager) |
| tokio-tungstenite, flate2/zstd-safe, reqwest + reqwest_cookie_store, rsa, sha2, uuid, iana-time-zone, sys-locale, image | permissive | n/a | Building blocks concord also uses (`Cargo.toml:15-95`) |

Crate data comes from the crates.io API (versions and licences, queried 2026-10-25) and the GitHub sources at twilight `46460a8` and serenity `current` [read].

#### 10.4 Recommendation

**Build our own MIT core**, informed by concord's behaviour and the community docs. Reuse ISC/MIT pieces: twilight `Id<T>`, optionally twilight's decompressor code, and generic crates. Don't depend on twilight or serenity as frameworks. They are bot-shaped, fight user-account requirements (token prefix, intents, READY model), and would need forking anyway [inferred].

---

### 11. Effort and LOC estimate (MVP text-only core)

**MVP scope:** token login, guilds/channels/DMs, history, live messages, send/reply/react/edit/delete, read state, images. Yardstick: concord non-test LOC per module (`wc -l`, inline tests included, so concord figures are slight overestimates).

| Module | Concord LOC | What the MVP needs | Est. MVP LOC |
|---|---|---|---|
| Gateway connection, zlib, session/resume, send window, op 8/13/37 (`gateway.rs`) | 2,561 (incl. voice/stream payloads, RPC activity encoding) | Everything except voice/stream/activity encoding; simpler op-8 scheduler | 1,200–1,600 |
| Dispatch parsers (`gateway/parser/*` minus voice/streams/interactions) | ~4,000 of 4,449 | READY/SUPPLEMENTAL, guild/channel/thread, message, reaction, typing, presence, member(+list), read state, relationships | 2,000–2,800 (typed serde can be shorter) |
| REST core: limiter, pacing, safety, errors (`rest.rs`) | 1,109 | All of it | 800–1,000 |
| REST endpoints: messages, reactions, read_state, channels, connection | ~990 | Same (skip pins/mentions/search/forum/profile/settings) | 500–700 |
| Fingerprint (UA, super-props, build no., cookies, experiments) | 924 | All of it | 600–800 |
| Auth: token validation + keyring store | ~450 (connection + token_store) | Token only | 250–350 |
| (Optional) email/password + MFA, QR | 420 + 621 | Phase 2 | +700–900 |
| State root + areas + snapshot/revision (`state.rs`, `state/*`) | 2,459 | Most of it | 1,200–1,600 |
| Domain state: message/channel/guild/member+list/read/notification/thread | ~6,230 | Message timelines, channels, guilds, members, member list, read state; minimal threads | 3,000–4,000 |
| Models/events (`message.rs`, `channel.rs`, `events.rs`, …) | ~5,177 | Typed models for the MVP set | 2,000–2,800 |
| Client facade + request lifecycle (`client*`, `request_lifecycle*`) | 2,616 + 1,288 | Facade, history/ack/member dedupe | 1,200–1,600 |
| Permissions + action/message policy (+ verification optional) | ~2,249 | Permission calculation, send/react/delete checks, DM safety locks | 900–1,300 |
| Media core: CDN/proxy URL builders + fetch/decode/LRU (GUI-agnostic) | TUI media 5,808 + adapters 743 (TUI-specific) | URL builders, pooled fetch, decode, byte-budgeted cache | 800–1,400 |
| **Total (non-test)** | ~37k text-relevant | | **≈14k–20k LOC**, plus tests ≈0.5–1× (concord's ratio is about 0.3) |

- **Effort [inferred]:** one experienced Rust engineer would need roughly **10–16 weeks** for a robust MVP core with tests. The gateway/state/REST skeleton could work in ~4–6 weeks; most of the remaining time goes to edge cases (resume/reidentify, member-list refresh, read-state versions, rate-limit learning, captcha and error UX). Email/MFA and QR add about 1–2 weeks.
- **Voice/screen-share (excluded):** concord's `src/discord/voice/` is **32,465 non-test LOC + 3,362 test LOC**. It covers the voice gateway, RTP/Opus, DAVE E2EE (`davey`), capture, H.264 encoders on three OSes, and playback. Separately, RPC/IPC rich presence is 2,054 LOC. Voice alone is roughly 2× the whole text MVP.

---

### Sources

#### Concord (GPL-3.0-only; read-only clone, HEAD b312f18)

- `README.md` (FAQ 1164-1177, Security 1179-1184, Performance 1154-1162), `Cargo.toml`, `CHANGELOG.md`
- `src/discord/gateway.rs`, `src/discord/gateway/parser.rs`, `parser/ready.rs`, `parser/members.rs`, `parser/read_state.rs`
- `src/discord/fingerprint.rs`, `client.rs`, `client/lifecycle.rs`, `client/rest_actions.rs`
- `src/discord/state.rs`, `state/caches.rs`, `state/snapshot.rs`, `message/state.rs`, `member/list.rs`
- `src/discord/rest.rs`, `rest/messages.rs`, `rest/reactions.rs`, `rest/read_state.rs`, `rest/channels.rs`, `rest/connection.rs`
- `src/discord/request_lifecycle.rs`, `request_lifecycle/primitives.rs`
- `src/discord/auth_http.rs`, `password_auth.rs`, `qr_auth.rs`, `captcha.rs`
- `src/discord/action_policy.rs`, `message_policy.rs`, `verification.rs`, `channel/state.rs`, `ids.rs`, `presence.rs`, `commands.rs`, `read/state.rs`, `avatar.rs`, `emoji.rs`, `message.rs`
- `src/support/token_store.rs`
- `src/tui/state/subscriptions.rs`, `tui/state/channels.rs`, `tui/state/composer/state.rs`, `tui/state.rs`
- `src/tui/media/{targets,protocol,decode,cache,preview,avatar,emoji,work}.rs`
- `src/app/{history_commands,read_state_commands,media_adapters}.rs`

#### Official (Discord)

- Gateway: https://docs.discord.com/developers/events/gateway
- Opcodes and close codes: https://docs.discord.com/developers/topics/opcodes-and-status-codes
- API reference, versions, image formatting, snowflakes: https://docs.discord.com/developers/reference
- Rate limits: https://docs.discord.com/developers/topics/rate-limits
- Message resource: https://docs.discord.com/developers/resources/message
- Terms of Service (effective 2025-09-29): https://discord.com/terms
- Self-bots support article: https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots
- "Protecting Your Data": https://support.discord.com/hc/en-us/articles/360039598252
- Developer Policy (effective 2024-07-08): https://support-dev.discord.com/hc/en-us/articles/8563934450327-Discord-Developer-Policy
- Developer Terms of Service: https://support-dev.discord.com/hc/en-us/articles/8562894815383-Discord-Developer-Terms-of-Service
- Engineering blog, zstd and READY size: https://discord.com/blog/how-discord-reduced-websocket-traffic-by-40-percent

#### Community-maintained (unofficial)

- docs.discord.food (discord-userdoccers):
  - https://docs.discord.food/gateway/using-gateway (capabilities, compression, limits)
  - https://docs.discord.food/topics/gateway-events (Identify, client_state, READY, READY_SUPPLEMENTAL, op 8, op 13)
  - https://docs.discord.food/reference (headers, super-properties, launch_signature, CDN/media proxy)
  - https://docs.discord.food/authentication
  - https://docs.discord.food/remote-authentication/desktop
  - https://docs.discord.food/topics/captcha-handling
  - https://docs.discord.food/topics/cloud-uploads
  - https://docs.discord.food/topics/read-state
  - https://docs.discord.food/topics/experiments
  - Source repo: https://github.com/discord-userdoccers/discord-userdoccers
- discord.py-self (MIT) @ `2ba64a9a997e151a9c259984e0a179b1fdf4aff4`:
  - https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/gateway.py (opcodes 14/37, payload builders)
  - https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/state.py (GuildSubscriptions docstring, range logic, 15 KiB limit)
  - https://github.com/dolfies/discord.py-self/blob/2ba64a9a997e151a9c259984e0a179b1fdf4aff4/discord/types/gateway.py (subscription payload types)
  - https://discordpy-self.readthedocs.io/en/latest/guild_subscriptions.html

#### Rust crates

- twilight @ `46460a801c10324341bf48b1719ecd1c36b7c6cd` (ISC): https://github.com/twilight-rs/twilight
  - `twilight-http/src/client/{mod,builder}.rs`
  - `twilight-gateway/src/{shard,lib,compression}.rs`, `Cargo.toml`
  - `twilight-model/src/gateway/payload/incoming/ready.rs`
- serenity (ISC), branch `current`: https://github.com/serenity-rs/serenity (`CHANGELOG.md`, `src/http/client.rs`)
- crates.io API for versions and licences: https://crates.io/api/v1/crates/{twilight-gateway,twilight-http,twilight-model,serenity,serenity_self,diself,discord-user-rs,discord-selfbot,hubbub,keyring}
- keyring 4.2.0 source (`src/v1.rs`, `Cargo.toml`) from https://static.crates.io/crates/keyring/keyring-4.2.0.crate; docs https://docs.rs/keyring/latest/keyring/
- Microsoft, CREDENTIALW limits: https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw
