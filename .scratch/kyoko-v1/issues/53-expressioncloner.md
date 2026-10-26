# 53 — ExpressionCloner

**What to build:** Users can right-click an emoji or sticker and copy it into a server they manage.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 28 (Message actions and context menus)

**Status:** resolved

- [x] A context-menu item slot exists
- [x] Cloning is offered only where the user has permission
- [x] The upload runs only from the user's click, and is paced and rate-limited

## Answer

Merged to main as `8234b05..d6367de`. Docs: `docs/plugins.md` ("Menu items", ExpressionCloner), `docs/ui.md`, `docs/rest.md`, `docs/locks.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md`.

- Menu slot `ui:message.menu`: a plugin declares items in `plugin.toml` under `[[menu]]` (`slot`, `key`, `label`, optional `icon` SVG from its own folder, `when` conditions). Kyoko evaluates the conditions from its own data each time a menu opens and never asks the plugin; conditions are `custom_emoji`, `image_sticker`, `can_create_expressions`, an unknown one never holds. Items come after kyoko's own behind a separator in the timeline menu, thread pane, hover-bar More menu and search results, and in a menu on the right-clicked cell in the emoji and sticker pickers. A choice goes through `PluginHost::menu_gesture`.
- ExpressionCloner (`plugins/expression-cloner/`, bundled, ships disabled): "Clone Emoji" / "Clone Sticker" on custom emoji and image stickers (never Unicode emoji or Lottie stickers), only with Create or Manage Expressions in at least one server. `kyoko.actions.clone_expression()` needs `action:upload_expression`, works only inside a menu gesture, counts against the per-plugin action limit, and only clones what was right-clicked. The dialog (`Dialog::Clone`, in the community dialog layer, not a new root layer) lists permitted servers with free slots; a full server is dimmed with the reason (still and animated emoji count separately). The upload is the user's click on a server: one `CreateEmoji { image }` / `CreateSticker { file }` (content type sniffed from the bytes, `filename` dropped) through the per-server write queue, one clone at a time; the image comes through `MediaService::bytes` (animated emoji as GIF). The session re-checks permission, type, size (256 KB emoji, 512 KB sticker), name rules and slots per boost tier before any request; server settings now shows tier slot counts and uses the same validation. Expression creation is not in the risky-action lock set (`docs/locks.md`). Emoji body is `{name, image}` without `roles`. mockcord marks emoji created from a GIF as animated.
- Tests: 6 session-seam (`session/tests/session/uploads.rs`), 4 plugin-seam (`expression_cloner.rs`), 3 bridge tests with the real host (`bridge/tests/bridge/cloner.rs`), an in-process `TestApp` test (right-click emoji → Clone Emoji → pick server), `headless-run` clones an emoji (`clone_ok`). Checks re-run by the orchestrator in lane C: all green; `community::forwards_are_paced_sends_and_respect_locks` and `storage::turning_the_opt_in_on_and_off_moves_the_secrets` failed under load and passed alone (known flakes).
- Parity (port 8493, `KYOKO_CAPTURE_PLUGINS=expression-cloner`, kyoko only): new `message-menu-clone` and `clone-dialog`, checked by eye. Plugin off, `message-context-menu` items match oxidcord (15.1%, emoji-picker 26.9%: the Midnight tokens and Inter, not this ticket).
- For later tickets: add a menu condition as a constant and a branch in `MenuFacts::holds` (`bridge/src/cloner.rs`) and document it under "Menu items". Right-click targets are marked with `timeline::actions::aimed(view, actions, Some(expression))`. "Ask the window to open X" goes through `PluginHost::on_request` / `PluginRequest`. M07: plugin items use `timeline::menu_row`; keep the `plugin-menu-<key>`, `expression-<id>` and `clone-server-<guild>` ids the tests use; plugin items stay last. `TestApp` tests that open menus or dialogs: the window is 400×300, call `set_window`, keep menus short, deliver frames (`app.frames(30)`) before clicking inside a modal.
- Real-Discord confirmation open: whether the web client sends `roles: []` on emoji create.
