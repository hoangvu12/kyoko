# 11 — Command choices, autocomplete and file options

**What to build:** Options with choices list them above the hint bar; options with autocomplete send the application-command autocomplete interaction as the web client does and show "Loading options...", the results, or "Couldn't load options" with Retry. File options show the drop box and then the picked file in the chip, and upload the file with the interaction as the web client does.

**Blocked by:** 10

**Status:** resolved

**Mockups:** `docs/design/mockups/command-choices-and-autocomplete.html`, `docs/design/mockups/command-choices-loading.html`, `docs/design/mockups/command-choices-failed.html`, `docs/design/mockups/command-attachment-option.html`, `docs/design/mockups/command-attachment-option-picked.html`

**Spec:** `docs/design/midnight.md`, Command picker and options

- [x] Autocomplete interactions are paced like other interactions and debounced while typing
- [x] A file option uploads and runs against mockbot (mockcord gains an autocomplete command and a file command)
- [x] Session-seam tests cover autocomplete results, failure and the file option

## Answer

Merged to main as `ec7185a`. Docs: `docs/rest.md`, `docs/ui.md`, `docs/testing.md`, `docs/parity.md`.

- Choices: a focused option with fixed choices lists them above the hint bar (the `M.optional` slot): option name in capitals as header, 36-high rows, filtered as typed, Up/Down, Enter/Tab/click picks; the chip shows the choice's name, the value is sent, focus moves to the next option. A picked last option keeps focus and the list then shows only that choice.
- Autocomplete: `Command::AutocompleteCommand` when an autocompleting option is focused or its value changes (once per value); the session marks Loading at once, waits 300 ms and sends only the newest, as a type 4 `POST /interactions` through the existing interaction pacer. Body = the run body without `analytics_location`, options typed so far, the edited one `focused: true`. Answer matched by nonce on `APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE`; REST failure, no answer in 5 s or a channel lock marks it failed (the lock case quietly). Result in `State::autocomplete` (newest only), flags `Changes::commands`. UI: "Loading options..." with dots, the choices, or "Couldn't load options" with Retry; zero choices hide the list.
- File options: a 72-high dashed drop box when focused (click opens the file dialog; a drop on the window fills the option instead of the upload tray); the picked chip shows thumbnail, name and remove (Backspace too); checked against the upload limit. `RunApplicationCommand { …, files }` reuses the message upload path (`POST /channels/{id}/attachments` slots, PUT to the upload URL), then posts the interaction with `data.attachments: [{id:"0", filename, uploaded_filename}]` and the option value as the index. **Unconfirmed against the web client** (needs a real command run on the user's account; a mismatch would be refused, not misbehave; noted in `docs/rest.md`).
- Mockcord: `/echo language` has the mockup's six languages; new Mockbot `/translate text language` (autocomplete, id …105) and Snapper `/resize file width` (id …402, replies "Resized cat.png to 200 pixels wide"); flags `--slow-autocomplete` (10 min), `--failing-autocomplete` (500); `Control::set_autocomplete`.
- Tests: session `command_options.rs`: `typing_is_debounced_into_one_autocomplete_paced_like_other_interactions`, `autocomplete_sends_the_focused_value_and_brings_back_the_choices`, `a_failed_autocomplete_says_so_and_a_retry_loads_the_choices` (500, failing mode, stalled until the 5 s timeout), `a_file_option_uploads_to_a_slot_then_runs_with_the_attachment`; bridge `choices_filter_as_typed_autocomplete_asks_once_per_value_and_files_ride_along`; load check `ui::scripts::the_command_choice_and_file_option_states_load`.
- Checks re-run by the orchestrator in lane A on top of M23: all green (`storage::turning_the_opt_in_on_and_off_moves_the_secrets` flaked once in `xtask test`, the rerun passed). Mockup parity (port 8494): command-choices-and-autocomplete 12.0%, -loading 12.7%, -failed 12.8%, command-attachment-option 12.7%, -picked 13.2%; M10 screens unchanged (picker 11.5%, options 13.1%, invalid 13.2%). The loading and failed screens use `/translate` instead of the mockup's `/echo`.
- For M12–M15: `RunApplicationCommand.files: Vec<UploadFile>`, `InteractionOption.focused: Option<bool>`; `session::task::interactions::interaction_body` takes an `Interaction { kind, … }` so it can build component (type 3) and modal (type 5) interactions; `Interactions::remember`/`settle` nonce bookkeeping is reusable. Bridge `ActiveCommand` gained `choices(autocomplete)`, `autocomplete(retry)`, `pick_choice`, `attach`, `clear_file`, `files()`, `wants_file()`; `OptionField.choice`/`file`; one request-id counter for both composers. Luau shared fields inside `composer.command` (`choices`, `wants_file`, per-field `takes_file`/`file`); actions `choose_option`, `retry_choices`, `option_files`, `remove_option_file`. M13: select menus can reuse `command_picker.choices` (`choice_status`, the dots). M14: `/gif`, `/sticker` options could use the same choices slot.
