# Knoveyla

Knoveyla is a local-first personal context assistant for Apple Silicon Macs. The
technical alpha records foreground application activity and explicitly permitted
Chrome metadata, keeps detailed activity in a local SQLite database, and sends a
minimized digest or an active chat conversation directly to a user-selected AI
provider.

This repository implements the alpha described in
[`knoveyla_prd.md`](knoveyla_prd.md). It is not a production-ready release.

## Alpha status

Implemented and usable from source:

- Tauri 2 desktop app with React, TypeScript, Rust, and SQLite
- macOS foreground app and active-window-title collection
- explicit Chrome-profile selection and up-to-90-day history bootstrap
- 30-day detailed-activity retention and post-bootstrap cleanup
- Chrome Manifest V3 companion extension with active-tab timing
- OpenAI and Anthropic BYOK credentials through macOS Keychain
- direct provider-backed profile refresh, recommendations, and chat
- dashboard, activity history, profile corrections, pause, and delete controls

Important alpha limitations:

- macOS 26 on Apple Silicon is the tested target. Safari, Firefox, Intel Macs,
  Windows, and Linux are not implemented release targets.
- Accessibility permission is required for window titles.
- Source builds still require loading the extension unpacked; Settings can
  register its Native Messaging host after the helper is built.
- Desktop collection state is synchronized to the extension; exclusion lists
  remain source-specific.
- Launch at login is user-controlled in Settings.
- The behavioral-guidance preference suppresses behavioral recommendations.
- Basic time/page activity insights are implemented, but topic/content
  categorization and some PRD control surfaces remain incomplete. Browser
  preview mode uses sample data.

See [Alpha setup](docs/alpha-setup.md) for the complete setup and limitation
notes.

## Developer quick start

Requirements:

- Apple Silicon Mac running macOS 26
- Xcode Command Line Tools
- Node.js 20.19+ or 22.12+ and npm
- current stable Rust toolchain
- Google Chrome 120+ for the companion extension

From the repository root:

```sh
npm install
npm run dev:desktop
```

The first command installs both npm workspaces. The second starts the Vite
frontend inside the native Tauri application. Running `npm run dev` starts only
the browser preview, which uses mock data and cannot collect activity, access
Keychain, or call native commands.

In the first-run wizard:

1. Review the data disclosure.
2. Open macOS Accessibility settings if you want window titles.
3. Select at least one detected Chrome profile.
4. Choose OpenAI or Anthropic and enter your API key.
5. Build the first profile.

The application stores an entered API key in macOS Keychain. For later
source-only sessions, the native provider client can override that credential
with `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` from its environment:

```sh
OPENAI_API_KEY="your-key" npm run dev:desktop
```

The first-run wizard still requires entering a key to configure the selected
provider. Knoveyla does not load `.env` files automatically.

## Verification

```sh
npm run typecheck
npm test
npm run build
npm run check:rust
npm run test:rust
npm run build:desktop
```

The Chrome extension build is written to `apps/extension/dist`. Load that
directory as an unpacked extension only after following
[Chrome extension setup](docs/alpha-setup.md#chrome-extension-setup).
The desktop bundle is written to
`apps/desktop/src-tauri/target/release/bundle/macos/Knoveyla.app`. It is an
unsigned technical-alpha build; code signing and notarization are not included.

## Documentation

- [Alpha setup](docs/alpha-setup.md)
- [Architecture](docs/architecture.md)
- [Privacy model](docs/privacy-model.md)
- [Threat model](docs/threat-model.md)
- [Testing](docs/testing.md)
- [Product requirements](knoveyla_prd.md)

## License

This repository is currently `UNLICENSED`.
