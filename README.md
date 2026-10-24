<p align="center">
	<img src="assets/icon.png" width="128" height="128" alt="Kyoko icon">
</p>

<h1 align="center">Kyoko</h1>

<p align="center">A lightweight native Discord client for Windows, written in Rust.</p>

> **Kyoko is not an official Discord product. It is not made, endorsed, sponsored or supported by Discord Inc.**
>
> **Using a third-party client with a Discord user account can violate the Discord Terms of Service and Community Guidelines. Your account can be rate limited, flagged, locked, suspended or permanently banned, with or without warning. If you are not willing to lose the account, do not use this software with it.**
>
> **This software is in early development. It is provided as is, with no warranty of any kind, and the authors accept no liability for anything that happens as a result of using it. Read [Disclaimer](#disclaimer) before you build or run it.**

## Contents

- [What it is](#what-it-is)
- [Credits: built on oxidcord](#credits-built-on-oxidcord)
- [First run](#first-run)
- [Where your data lives](#where-your-data-lives)
- [Build from source](#build-from-source)
- [Disclaimer](#disclaimer)
- [Privacy and security](#privacy-and-security)
- [Contributing](#contributing)
- [License](#license)

## What it is

- A native window drawn with zgui, with the interface written in Luau scripts on top of a Rust core.
- Luau plugins with a declared, default-deny permission model.
- No telemetry, no analytics, no crash upload, no remote logging.
- Session token kept in the operating system credential store. The local message cache is encrypted with a key held in the same store.
- Windows only for v1.

There are no releases yet. Build from source to try it.

## Credits: built on oxidcord

Kyoko is a fork of [oxidcord](https://github.com/Master0fFate/oxidcord) by Master0fFate and the oxidcord contributors, forked at release 0.2.0-beta.1. Its protocol, gateway, REST, state, store, login, media, markdown and platform crates, the mock Discord server and the development tasks come from oxidcord, and oxidcord's copyright notice, licence files and [NOTICE](NOTICE) are kept as its MIT and Apache-2.0 terms require. Thank you.

What kyoko changes:

- **UI.** oxidcord's GPUI interface is replaced by a zgui window whose interface is written in Luau, with a Rust timeline view for the message list.
- **Core.** The inherited core gets a correctness and safety pass: request pacing and backoff, safer defaults, and behaviour tests at the session seam against the in-process mock server.
- **Plugins.** The WebAssembly plugin host is replaced by Luau plugins running in a sandbox with declared permissions.
- **Platforms.** v1 targets Windows only. Voice, the self-updater and the macOS and Linux packages are dropped.
- **Identity.** Kyoko uses its own name, app id, data folders and icon, and trusts none of oxidcord's signing keys or update and catalog endpoints.

Third-party data bundled with kyoko, such as the emoji shortcode table from GitHub's gemoji, is listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## First run

1. Start Kyoko. The login screen carries a notice about the Discord Terms of Service and account termination. Read it, and only continue if you accept the risk.
2. Sign in with one of the methods the login screen offers:
   - **QR code**: open Discord on your phone, go to Settings, Scan QR Code, and scan the code shown in Kyoko.
   - **Web login**: sign in through a Discord login page in a separate window. Kyoko only reads the resulting session.
   - **Token**: paste an existing session token. Never paste a token someone else gave you, and never share yours.
3. If the sign-in asks for multi-factor authentication or a captcha, complete it yourself. Kyoko does not solve, skip or bypass either of them.
4. Your servers and direct messages load in the sidebar. The token is saved to Windows Credential Manager.

Want to look around without an account? Run the built-in mock server. Nothing leaves your machine:

```bash
kyoko --mock
```

| Flag | Effect |
| --- | --- |
| `--mock` | Run against a local fake Discord server. |
| `--churn <seconds>` | With `--mock`, create a channel or server on the fake server every few seconds. |
| `--data-dir <path>` | Keep settings, cache and logs under this folder, for example on a USB stick. |
| `--log <filter>` | Log level, for example `debug`. Logs never contain the token or raw gateway payloads. |

## Where your data lives

| What | Windows |
| --- | --- |
| Data and cache database | `%APPDATA%\hoangvu12\Kyoko\data` |
| Settings | `%APPDATA%\hoangvu12\Kyoko\config` |
| Media cache | `%LOCALAPPDATA%\hoangvu12\Kyoko\cache` |

To remove everything, delete those folders and the `kyoko` entries from Windows Credential Manager. Signing out in the app removes the saved token.

## Build from source

You need the Rust toolchain pinned in `rust-toolchain.toml`, the MSVC build tools and CMake.

```bash
git clone https://github.com/hoangvu12/kyoko
cd kyoko
cargo run --locked --release -p kyoko -- --mock
```

The desktop UI is being rebuilt on zgui. For now the window shows the server list and the selected server's channels. `kyoko --mock` runs it against the mock server; `kyoko` without `--mock` signs in with the token in the `KYOKO_TOKEN` environment variable and keeps it in memory only. Without `--data-dir`, settings and cache go to a scratch folder that is deleted on exit. Debug builds reload the Luau interface in `apps/desktop/ui/` as you save it. Read `docs/architecture.md` for the crate layout and `CONTRIBUTING.md` before sending changes.

## Disclaimer

Please read all of this. By downloading, installing, building or running Kyoko you agree to it.

**1. Not affiliated.** Kyoko is an independent, unofficial project. It has no connection to Discord Inc. "Discord" and the Discord logo are trademarks of Discord Inc. They are used here only to say what the software is compatible with. The Kyoko name, icon and interface are not meant to look like, and must not be mistaken for, an official Discord product.

**2. Terms of Service and account risk.** Discord's Terms of Service and Developer Policy restrict the use of unofficial or modified clients with user accounts. Kyoko presents the client properties and request headers of Discord's web client, and that does not make it undetectable or permitted. Discord can tell clients apart by network fingerprint and by behavior. Using Kyoko can lead to warnings, feature restrictions, captchas, forced password resets, temporary or permanent account suspension, loss of purchases and subscriptions, loss of access to servers and data, and in the worst case a permanent ban. These decisions are Discord's alone. Nobody here can reverse them, appeal them for you or compensate you.

**3. Use is entirely at your own risk.** You alone decide whether to use Kyoko, with which account, and for what. Use a throwaway account if you are unsure. You are responsible for following the rules of Discord, of every server you join, and of the laws that apply to you.

**4. No warranty.** The software is provided "as is" and "as available", without warranty of any kind, express or implied, including merchantability, fitness for a particular purpose, non-infringement, accuracy, availability, security or freedom from bugs. This is early, unfinished software. It can crash, lose or corrupt local data, display messages incorrectly, fail to send or delete messages, or stop working without notice when Discord changes its service.

**5. No liability.** To the fullest extent permitted by law, the authors, maintainers and contributors are not liable for any claim, damage or loss arising from or connected to the software or its use. That includes, without limit, account suspension or termination, lost data, lost messages, lost money or digital goods, lost income, reputational harm, privacy incidents, token or account compromise, damage to your device, and any direct, indirect, incidental, special, consequential or punitive damages, even if advised that they were possible. If your jurisdiction does not allow some of these exclusions, they apply as far as the law lets them.

**6. You indemnify the authors.** You agree to hold the authors, maintainers and contributors harmless from claims by third parties, including Discord Inc., that come from your use of the software or your breach of someone else's terms or rights.

**7. Third-party code and content.** Plugins, themes, catalogs and links come from third parties. They are not reviewed for safety or legality in any binding way, a "reviewed" flag is not a guarantee, and you install them at your own risk. Messages and media you see come from Discord and other users, not from this project.

**8. No support, no guarantees of continuity.** There is no service level, no support obligation and no promise that releases or this repository will keep existing. A release can be pulled at any time, for example after a request from a rights holder.

**9. Not for abuse.** Kyoko is meant for reading and writing your own messages like any other chat client. It does not automate user actions and does not bypass captchas, multi-factor authentication, rate limits or client verification, and you may not use it, or modify it, to spam, harass, scrape, self-bot, mass-message, evade bans, or break the law. Anyone who does that does it on their own account and their own responsibility.

**10. Not legal, security or financial advice.** Nothing in this repository is legal advice. If a legal question matters to you, ask a lawyer in your jurisdiction.

**11. Your token is your account.** Anyone holding your session token can act as you. Never post it, never paste it into software you do not trust, and never attach it to a bug report. If you think it leaked, change your Discord password, which invalidates it.

If you do not agree with any of this, do not use Kyoko.

## Privacy and security

- No telemetry of any kind. The only network traffic is to Discord, to the public pages used to read the current Discord build number and Chrome version once a day, and to the media hosts Discord messages point at. Kyoko has no update server and no plugin or theme catalog yet, so it fetches neither.
- The token is attached only to `https://discord.com` requests and is dropped on any redirect. It is never logged.
- Everything from the network, from a plugin or from disk is treated as untrusted and decoded leniently.
- The message cache is encrypted at rest with XChaCha20-Poly1305. The key lives in the OS credential store, or in a file next to the settings if you turn on the plaintext storage setting, which shows a warning. The channel and message ids and timestamps used to index the cache stay unencrypted.

Report vulnerabilities privately as described in `SECURITY.md`, never in a public issue.

## Contributing

Read `AGENTS.md` and `CONTRIBUTING.md`. Other open source Discord clients may be read for protocol facts and ideas, but their source must not be copied into this project.

## License

Kyoko is dual licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option, like oxidcord, from which it is forked. Unless you state otherwise, any contribution you intentionally submit for inclusion in Kyoko, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.

Both licenses disclaim all warranties and limit liability, and the [Disclaimer](#disclaimer) above applies in addition to them. The licenses cover the source code of this repository. They give you no rights to Discord's name, marks, service or content. Third-party dependencies keep their own licenses, which `cargo deny check` verifies against the allow list in `deny.toml`. Bundled third-party data is listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
