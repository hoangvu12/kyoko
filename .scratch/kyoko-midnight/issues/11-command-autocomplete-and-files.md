# 11 — Command choices, autocomplete and file options

**What to build:** Options with choices list them above the hint bar; options with autocomplete send the application-command autocomplete interaction as the web client does and show "Loading options...", the results, or "Couldn't load options" with Retry. File options show the drop box and then the picked file in the chip, and upload the file with the interaction as the web client does.

**Blocked by:** 10

**Status:** claimed

**Mockups:** `docs/design/mockups/command-choices-and-autocomplete.html`, `docs/design/mockups/command-choices-loading.html`, `docs/design/mockups/command-choices-failed.html`, `docs/design/mockups/command-attachment-option.html`, `docs/design/mockups/command-attachment-option-picked.html`

**Spec:** `docs/design/midnight.md`, Command picker and options

- [ ] Autocomplete interactions are paced like other interactions and debounced while typing
- [ ] A file option uploads and runs against mockbot (mockcord gains an autocomplete command and a file command)
- [ ] Session-seam tests cover autocomplete results, failure and the file option
