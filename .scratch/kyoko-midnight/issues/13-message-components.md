# 13 — Message components and app modals

**What to build:** Draw and run message components: action rows of buttons (primary, secondary, success, danger, link, premium shown disabled, emoji, disabled, loading), string, user, role, mentionable and channel selects (single and multi), and Components V2 (container, section with a thumbnail or button accessory, text display, separator, media gallery, file). Pressing or choosing sends the component interaction as the web client does; a modal response opens the app modal (short and paragraph inputs, selects, required marks, counters) and submits it. Unknown component types are skipped, never failing the message (lenient decoding).

**Blocked by:** 12

**Status:** resolved

**Mockups:** `docs/design/mockups/message-components.html`, `docs/design/mockups/message-components-multi-select.html`, `docs/design/mockups/message-components-v2.html`, `docs/design/mockups/app-modal.html`, `docs/design/mockups/app-modal-error.html`

**Spec:** `docs/design/midnight.md`, Command replies and components

- [x] Components and modals match the mockups
- [x] Link buttons go through the "Leave Kyoko?" check like other links
- [x] Mockbot gains buttons, a select, a V2 message and a modal; session-seam tests cover each interaction and a failure
- [x] Unknown component types render nothing and are logged without payloads

## Answer

Merged to main as `59aa587`. Docs: `docs/rest.md` (component and modal interactions), `docs/ui.md`, `docs/testing.md` (Mockbot), `docs/parity.md`.

- Protocol (from Discord's public web bundle, UA `kyoko-dev`, no cookie or identity, facts only): a press or select is `POST /interactions` type 3 with `nonce`, `guild_id`, `channel_id`, `message_flags`, `message_id`, `application_id`, `session_id`, `data: {component_type, custom_id}` (selects add `type` and `values`), JSON not multipart, 3 s timeout; a second press on a message still waiting is dropped. Modal submit is type 5 with `data: {id, custom_id, components}` mirroring the modal's structure; success closes it, a failure or no success within 10 s shows "Something went wrong. Try again."; `INTERACTION_MODAL_CREATE` removes the pending command row (M12's leftover).
- Model: `protocol::Component` and `InteractionModal`; `Message.components` is `LenientVec<Component>`; unknown types decode as `Unknown(n)`, render nothing and are logged once per type without payload; an undecodable component is dropped without failing the message.
- Session: `Command::PressComponent`, `SubmitModal`, `CloseModal`; `session::task::components` keeps component and modal nonces apart from command nonces; failures set M12's `LocalEvent::ComponentInteraction` ("This interaction failed"). State: `State::app_modal()`.
- UI: `timeline/components.rs` draws buttons (every style, emoji, disabled, premium disabled, spinner while waiting), string/user/role/mentionable/channel selects single and multi (the list is a zgx popover anchored to the field, matching its width, over the messages below), and Components V2 (container, section with thumbnail or button, text display, separator, media gallery, file). `timeline/app_modal.rs`: 440 wide, app icon and name, required asterisks, paragraph counter, select field, missing-field message, error line, Cancel/Submit; fed by `bridge::components::ModalForm::of` and `TimelineView::set_modal`. Link buttons go through the existing "Leave Kyoko?" link check.
- Mockcord: Mockbot `/buttons`, `/fruit`, `/fruits`, `/release`, `/bug` ("Report a bug" modal), component and modal-submit responders, a `showcase: components` section before reactions, flag `--failing-modals`, `Control::post_components` for raw components. The parity tool passes `--slow-followups` for `message-components` and `--failing-modals` for `app-modal-error`.
- Tests: session `components.rs` (six: button press with the exact body, select choice, failed press including a refused request, unknown component type, modal open + missing required field refused with no request + successful submit, failed submit then Cancel); in-process `timeline::component_tests` (button press sends, link button opens the check with nothing reaching Discord, modal takes focus and typed text lands in Title).
- Checks re-run by the orchestrator in lane A on `d8eb152`: all green (fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny; luau0-src `0.22.0+luau740`); rebased cleanly onto 62. Mockup parity (port 8494, `pair.png` checked by eye): message-components 15.8%, -multi-select 15.9%, -v2 23.7% (mock gallery images differ in colour), app-modal 10.8%, app-modal-error 10.7%; command-response-states unchanged 14.9%.
- Deviations: component messages live in `#showcase` rather than coming from command replies (no "used /x" header); consecutive Mockbot messages group under one header; in `app-modal-error` the required "What happened?" is filled (kyoko refuses an empty required field locally); Mockbot's new `/bug` shifts the M10 picker list (bridge test updated, picker screens not recaptured). Not done: container spoilers aren't blurred, the spinner doesn't rotate, user/role/channel selects list only cached entries (up to 25), a 429 on modal submit isn't retried (the web client retries), file-upload, radio and checkbox modal fields aren't handled.
- For M14/M15: M12's unstaged "This interaction failed" row can be staged by pressing Danger on the showcase buttons; the select popover in `timeline/components.rs` may suit M15's apps launcher.
