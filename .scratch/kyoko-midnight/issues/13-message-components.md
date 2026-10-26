# 13 — Message components and app modals

**What to build:** Draw and run message components: action rows of buttons (primary, secondary, success, danger, link, premium shown disabled, emoji, disabled, loading), string, user, role, mentionable and channel selects (single and multi), and Components V2 (container, section with a thumbnail or button accessory, text display, separator, media gallery, file). Pressing or choosing sends the component interaction as the web client does; a modal response opens the app modal (short and paragraph inputs, selects, required marks, counters) and submits it. Unknown component types are skipped, never failing the message (lenient decoding).

**Blocked by:** 12

**Status:** claimed

**Mockups:** `docs/design/mockups/message-components.html`, `docs/design/mockups/message-components-multi-select.html`, `docs/design/mockups/message-components-v2.html`, `docs/design/mockups/app-modal.html`, `docs/design/mockups/app-modal-error.html`

**Spec:** `docs/design/midnight.md`, Command replies and components

- [ ] Components and modals match the mockups
- [ ] Link buttons go through the "Leave Kyoko?" check like other links
- [ ] Mockbot gains buttons, a select, a V2 message and a modal; session-seam tests cover each interaction and a failure
- [ ] Unknown component types render nothing and are logged without payloads
