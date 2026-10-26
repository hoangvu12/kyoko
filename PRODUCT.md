# Product

## Register

product

## Users

The maintainer first, then Discord users who want a lighter, faster native Windows client. They spend hours a day in Discord, chatting, moderating and moving between servers and DMs, and expect everything to be where Discord puts it.

## Product Purpose

Kyoko is Discord's own desktop layout and behaviour in a lightweight Rust app, styled with refact0r's Midnight theme (warm dark panels floating on a darker frame), with plugins and themes built in and sandboxed. Success: it feels like Discord, nothing the user relies on is missing, and it uses a fraction of the official client's memory.

## Brand Personality

Taken from the Midnight design (`docs/design/midnight.md`): faithful to Discord, warm and calm (Midnight's palette and soft motion), quietly careful (it never puts the account at risk).

## Anti-references

- Nitro upsells or any "upgrade" prompt; what needs Nitro is shown locked with the reason.
- Vencord's own extra UI (Spotify controls, member counts, translate buttons).
- The heaviness of the Electron client.
- Anything that automates user actions or bypasses Discord's checks (captchas, MFA, rate limits).
- Layouts that drift from Discord's.

## Design Principles

1. The design is the authority: where the Midnight mockups differ from older kyoko behaviour, follow the mockups.
2. Show what kyoko can do and leave out what it can't, rather than faking it.
3. Never act on the user's account without a real click.
4. Motion stays soft and always honours reduced motion.

## Accessibility & Inclusion

WCAG AA: contrast where Midnight's palette allows it, every action reachable by keyboard, the reduced-motion setting respected everywhere.
