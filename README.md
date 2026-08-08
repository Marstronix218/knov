# Knov

Knov is a local-first personal context assistant for Apple Silicon Macs. The
technical alpha records foreground application activity, explicitly permitted
Chrome metadata, and metadata-only editor changes. It keeps detailed activity in
a local SQLite database and sends only a minimized digest or token-budgeted chat
context directly to a user-selected AI provider.

This repository implements the alpha described in
[`knov_prd.md`](knov_prd.md). It is not a production-ready release.

## Screenshot

![Knov dashboard showing tracked time, application usage, web attention, and recent activity](docs/screenshots/dashboard.jpg)

## Alpha status

Implemented and usable from source:

- Tauri 2 desktop app with React, TypeScript, Rust, and SQLite
- macOS foreground app and active-window-title collection
- explicit Chrome-profile selection and up-to-90-day history bootstrap
- 30-day detailed-activity retention and post-bootstrap cleanup
- optional experimental Chrome Manifest V3 companion extension with active-tab timing
- metadata-only Local History and recent Git-path signals from supported editors
- semantic work threads across app, browser, document, and editor evidence
- privacy-safe link-only resource previews and one-click thread resumption
- deterministic, sanitized context packing with local context-economics metrics
- OpenAI, Anthropic, and Amazon Bedrock BYOK credentials through macOS Keychain
- direct provider-backed profile refresh, recommendations, and chat
- dashboard, activity history, profile corrections, pause, and delete controls

Important alpha limitations:

- macOS 26 on Apple Silicon is the tested target. Safari, Firefox, Intel Macs,
  Windows, and Linux are not implemented release targets.
- Accessibility permission is required for window titles.
- The optional companion is a post-MVP experiment and must be loaded unpacked;
  its Native Messaging host is registered manually for compatibility testing.
- Desktop collection state is synchronized to the extension; exclusion lists
  remain source-specific.
- Launch at login is user-controlled in Settings.
- The behavioral-guidance preference suppresses behavioral recommendations.
- Basic time/page activity insights are implemented, but topic/content
  categorization and some PRD control surfaces remain incomplete. Browser
  preview mode uses sample data.

See [Alpha setup](docs/alpha-setup.md) for the complete setup and limitation
notes.

## Install the app

Requirements:

- Apple Silicon Mac running macOS 26
- Git
- Xcode Command Line Tools
- Node.js 20.19+ or 22.12+ and npm
- current stable Rust toolchain
- Google Chrome (required for selected-profile history import; Chrome 120+ for
  the optional companion extension)

### 1. Download the source

Clone this repository and enter its directory:

```sh
git clone https://github.com/Marstronix218/knov.git
cd knov
```

If you already downloaded the repository as a ZIP, extract it, open Terminal,
type `cd ` with a trailing space, drag the extracted `knov` folder into the
Terminal window, and press Return.

### 2. Install dependencies

From the `knov` repository root:

```sh
npm install
```

This installs the desktop and Chrome-extension npm workspaces. Rust downloads
and compiles the native dependencies the first time the desktop app is started
or built.

### 3. Choose how to run Knov

For the quickest source-development launch:

```sh
npm run dev:desktop
```

This starts the Vite frontend inside the native Tauri application. Keep the
terminal open while using Knov; stopping the process also stops activity
collection and the local Chrome bridge.

If Tauri reports that `cargo metadata` failed with `No such file or directory`,
Rust's tools are not available in the current shell. Restart Terminal or reload
the environment installed by `rustup`, confirm Cargo is available, and retry:

```sh
source "$HOME/.cargo/env"
cargo --version
npm run dev:desktop
```

If this recurs in zsh, add `. "$HOME/.cargo/env"` to `~/.zprofile`.

To create an installable macOS application instead:

```sh
npm run build:desktop
open apps/desktop/src-tauri/target/release/bundle/macos
```

When Finder opens, drag **Knov.app** into **Applications**, then launch it
from that folder. This technical-alpha bundle is unsigned and not notarized. If
macOS blocks the first launch, Control-click **Knov.app**, choose **Open**,
and confirm that you want to open it.

Running `npm run dev` starts only the browser preview. It uses mock data and
cannot collect activity, access Keychain, or call native commands, so it is not
a substitute for the native app.

## First-time setup

Knov opens a four-step setup wizard on its first native launch:

1. **Welcome:** review what Knov collects and how the data is handled.
2. **Permissions:** choose **Open macOS permission prompt** if you want active
   window titles. In **System Settings → Privacy & Security → Accessibility**,
   enable the running Knov development process. App-duration tracking still
   works without this permission, but window-title context is unavailable. If
   the permission does not take effect immediately, restart the app.
3. **Browser profiles:** select at least one detected Chrome profile. Knov
   temporarily imports up to 90 days of history to build the initial profile;
   history older than 30 days is removed after that profile succeeds.
4. **AI provider:** select OpenAI, Anthropic, or Amazon Bedrock, paste an API key, and choose
   **Build my first profile**. The key is stored in macOS Keychain. Building the
   initial profile requires a working key and an internet connection to the
   selected provider.

When setup finishes, confirm that the sidebar says **Collection active**. Use
**Resume** if collection is paused.

For later source-development sessions, the native provider client can override
the Keychain credential with `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or
`AWS_BEDROCK_API_KEY` from its environment:

```sh
OPENAI_API_KEY="your-key" npm run dev:desktop
```

The first-run wizard still requires entering a key to configure the selected
provider. Knov does not load `.env` files automatically.

## Optional: connect the experimental Chrome companion

The MVP does not require the extension: onboarding, history bootstrap, and
foreground app/window collection work through the desktop app alone. Install
the implemented companion only when developing or evaluating the post-MVP
active-tab timing enhancement.

1. Build the extension from the repository root:

   ```sh
   npm run build --workspace @knov/chrome-extension
   ```

2. Build the Native Messaging helper used by the development app:

   ```sh
   cargo build \
     --manifest-path apps/desktop/src-tauri/Cargo.toml \
     --bin knov-native-host
   ```

3. Open `chrome://extensions`, enable **Developer mode**, choose **Load
   unpacked**, and select `apps/extension/dist`.
4. Copy the 32-character ID from the Knov extension card.
5. Follow the manual Native Messaging registration in
   [Alpha setup](docs/alpha-setup.md#3-register-the-helper-with-chrome).
6. Start the compatibility build with `npm run dev:with-extension`, then restart
   Chrome so it sees the Native Messaging registration.
7. Copy the pairing token as described in Alpha setup and the approved profile
   ID shown under desktop **Settings → Browser profiles**.
8. Open the extension's **Details → Extension options** page. Keep
   **Native Messaging (recommended)** selected, paste the pairing token and
   approved Chrome profile ID, then choose **Save and verify**.
9. Open the extension popup. A successful setup shows **Collection is on** and
   a connected local-app status.

Repeat the load and pairing steps for each Chrome profile you want to approve.
You do not need to register the Native Messaging host again when Chrome shows
the same extension ID. If Chrome assigns a new ID after the extension is
reloaded or reinstalled, register the new ID again; the alpha host manifest
authorizes one extension ID at a time. See
[Chrome extension setup](docs/alpha-setup.md#optional-chrome-extension-setup) for manual
Native Messaging registration and the development-only local HTTP fallback.

## Use Knov

### Control collection

The card at the bottom of the sidebar always shows the current desktop
collection state. Choose **Pause** to stop storing new activity or **Resume** to
start again. The **Collection active** toggle in Settings controls the same
state. The Chrome companion observes a desktop pause on its next status check;
after resuming in the desktop app, also check the extension popup and resume it
there if it remains paused.

The extension popup shows its connection, collection state, and the page
currently being timed. Its **Pause collection** button is useful when you want
to stop browser collection directly.

### Resume work from Now

Open **Now** to see the work thread Knov believes you are most likely to
continue, the local evidence behind it, and a suggested next move. Choose
**Resume thread** to reopen its latest available web resource, **Ask with
context** to start a provider-backed conversation with an inspectable context
packet, or **Copy brief** to use that context elsewhere. Knov sanitizes and
packs the selected evidence under a token budget; full URLs, local absolute
paths, credential-like fields, and unrelated raw activity are not attached.

Choose another active thread to change the focal context. Open **Attention
details** when you want supporting app, web, timeline, and pattern analytics.
Use **Today**, **7 days**, or **30 days** to change the reporting period, and
the refresh icon to rebuild the profile and recommendations.

### Review Threads

Open **Threads** to inspect the provisional work streams Knov reconstructs from
activity. Selecting a thread shows its summary, suggested next move, and exact
available evidence. Repeated subjects can join one thread across searches,
videos, sites, documents, and supported editor metadata. Thread groupings are
inferences rather than confirmed user intent.

### Inspect the Activity timeline

Open **Activity** to inspect individual records. Each row shows the time, page
or window title, application, source, and duration. The source labels distinguish
desktop collection (`collector`), imported Chrome history (`history`), and live
extension activity (`chrome`), and metadata-only editor changes (`editor`).

Change the date range or use **Filter apps, pages, or topics** to narrow the
timeline. Detailed activity is retained locally for 30 days.

### Correct Memory

Open **Memory** to review Knov's current understanding:

- **inferred** items were generated from activity and can be hidden with the
  close button;
- **observed** items come directly from recorded activity; and
- **user** items are authoritative corrections that override inference.

Choose **Add correction** to save something Knov should treat as true.
User corrections can later be edited or removed. Use **Edit summary** to replace
the generated profile summary with your own text, or **Clear** to remove the
saved summary.

### Ask with context

Choose **Ask with context** from Now to review the candidate context, enter a
question, and choose **Send**. Knov retrieves relevant profile facts locally,
adds query-specific activity aggregates, and deterministically packs sanitized
selected-thread evidence under the configured token budget. The assistant shows
the sent context, the larger local comparison baseline, token savings, provider
usage, and locally stored run metrics. Chat history is not persisted.

### Configure Settings

Use **Settings** to:

- switch between OpenAI, Anthropic, and Amazon Bedrock, save or remove the selected provider's
  Keychain credential, and run **Test connection**;
- enable or disable collection, behavioral break/focus guidance, and launch at
  login;
- inspect Accessibility and Chrome connection diagnostics and the local
  database path;
- approve or remove Chrome profiles;
- register the Chrome Native Messaging host; and
- manage exclusions and deletion.

## Privacy controls and deletion

Under **Settings → Exclusions**, enter comma-separated application names and
domains, then choose **Save exclusions**. Matching desktop activity is dropped
locally before it can affect the profile. Add browser domains separately in the
extension settings, one domain per line; a rule such as `example.com` also
excludes its subdomains.

To reset Knov, use **Settings → Delete Knov data → Delete everything**.
This permanently removes app-owned activity, profiles, corrections,
recommendations, settings, provider credentials, and the Native Messaging
manifest, then rotates the pairing token. It does not remove the unpacked Chrome
extension or clear the extension's local settings; remove the extension from
`chrome://extensions` to clear those.

## Troubleshooting

- **The browser preview contains sample data:** launch with
  `npm run dev:desktop`; `npm run dev` is a frontend-only preview.
- **Window titles are missing:** grant Accessibility access in macOS System
  Settings, then restart the Tauri app.
- **No Chrome profiles appear:** install and open Chrome at least once, make
  sure the desired profile exists locally, and relaunch Knov.
- **The extension is disconnected:** keep the desktop app running, confirm the
  pairing token and approved profile ID, re-register the current 32-character
  extension ID, restart Chrome, and choose **Test connection** in extension
  settings.
- **Provider actions fail:** open **Settings → AI provider**, confirm the
  selected provider has the correct key, and choose **Test connection**.
- **No new activity appears:** confirm the sidebar and extension both show
  collection on, then check the desktop and extension exclusion lists.

## Verification

```sh
npm run typecheck --workspace @knov/desktop
npm test --workspace @knov/desktop
npm run build --workspace @knov/desktop
npm run check:rust
npm run test:rust
npm run build:desktop
```

These are the baseline desktop MVP checks. The optional extension compatibility
lane is documented in [Testing](docs/testing.md#optional-extension-compatibility-lane).
Its build is written to `apps/extension/dist`; load that directory unpacked only
after following [Chrome extension setup](docs/alpha-setup.md#optional-chrome-extension-setup).
The desktop bundle is written to
`apps/desktop/src-tauri/target/release/bundle/macos/Knov.app`. It is an
unsigned technical-alpha build; code signing and notarization are not included.

## Documentation

- [Alpha setup](docs/alpha-setup.md)
- [Architecture](docs/architecture.md)
- [Privacy model](docs/privacy-model.md)
- [Threat model](docs/threat-model.md)
- [Testing](docs/testing.md)
- [Product requirements](knov_prd.md)

## License

This repository is currently `UNLICENSED`.
