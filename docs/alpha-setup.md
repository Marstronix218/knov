# Alpha setup

## Supported environment

The tested target is an Apple Silicon Mac running macOS 26 with Google Chrome
120 or newer. This source alpha is not signed, notarized, or packaged for
consumer installation.

Release builds are universal with a macOS 13 minimum, so Knov may launch on
Intel Macs or older macOS. That is untested and unsupported.

Install:

- Xcode Command Line Tools (`xcode-select --install` if absent)
- Node.js 20.19+ or 22.12+ and npm
- current stable Rust through `rustup`
- Google Chrome
- `sqlite3` for optional local-state diagnostics (included with macOS)

Confirm the local toolchain:

```sh
node --version
npm --version
rustc --version
cargo --version
xcode-select -p
uname -m
```

`uname -m` should print `arm64`.

## Run the desktop app

From the repository root:

```sh
npm install
npm run dev:desktop
```

The first launch creates the SQLite database in the Tauri application-data
directory. Launch at login is off by default; turn it on in Settings. Complete
the setup wizard. An AI provider is optional: choose **Skip AI and finish** to
start with local features only, and connect an AI later in Settings. Without
one, AI features (Ask, the AI interview, and provider-backed profile refresh)
are unavailable, but collection and the dashboard work. After onboarding, use
the sidebar control to resume live desktop collection.

### macOS Accessibility

The setup wizard can open:

**System Settings → Privacy & Security → Accessibility**

Allow the running Knov development process when prompted. Without
Accessibility permission, collection is degraded: the app may identify
foreground applications, but it cannot reliably read active-window titles.
Permission does not give Knov a feature for recording keys, screenshots,
clipboard data, or document bodies.

After changing permission, restart the Tauri app if macOS does not apply it to
the active development process.

## BYOK provider setup

Choose **On this Mac** to use a local Ollama or LM Studio model (no key;
only loopback addresses are accepted), or **Cloud API key** for OpenAI,
Anthropic, or AWS Bedrock, in onboarding or Settings. For a cloud provider,
paste an API key. Save stores the key in macOS Keychain; **Test connection**
makes a direct request to the selected provider.

After a provider has been configured through the app, a source-development
session can override its Keychain credential through the environment:

```sh
ANTHROPIC_API_KEY="your-key" npm run dev:desktop
```

An environment value takes precedence over Keychain, but it does not configure
the initial provider selection by itself. The first-run wizard still requires an
entered key. Do not commit keys. Knov does not automatically read `.env`
files.

## Optional Chrome extension setup

This section is an optional compatibility/developer lane for the implemented
post-MVP active-tab experiment. Skip it for baseline MVP setup: desktop
onboarding, Chrome history import, and foreground app/window collection do not
depend on extension installation or pairing.

### 1. Build and load the extension

From the repository root:

```sh
npm run build --workspace @knov/chrome-extension
```

In Chrome:

1. Open `chrome://extensions`.
2. Enable **Developer mode**.
3. Choose **Load unpacked**.
4. Select the repository's `apps/extension/dist` directory.
5. Copy the 32-character extension ID shown on its card.

Keep the ID. An unpacked extension's ID may change if Chrome treats it as a new
installation, in which case the Native Messaging manifest must be regenerated.

### 2. Build the Native Messaging helper

```sh
cargo build \
  --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --bin knov-native-host
```

### 3. Register the helper with Chrome

Run the following from the repository root after replacing the extension ID.
The Node command writes only Chrome's per-user Native Messaging manifest.

```sh
KNOV_REPO="$(pwd)"
KNOV_EXTENSION_ID="replace-with-the-32-character-id"
KNOV_HOST="$KNOV_REPO/apps/desktop/src-tauri/target/debug/knov-native-host"
KNOV_MANIFEST_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"

mkdir -p "$KNOV_MANIFEST_DIR"
node -e '
const fs = require("fs");
const [path, host, extensionId] = process.argv.slice(1);
fs.writeFileSync(path, JSON.stringify({
  name: "com.knov.companion",
  description: "Knov local activity bridge",
  path: host,
  type: "stdio",
  allowed_origins: [`chrome-extension://${extensionId}/`]
}, null, 2) + "\n");
' \
  "$KNOV_MANIFEST_DIR/com.knov.companion.json" \
  "$KNOV_HOST" \
  "$KNOV_EXTENSION_ID"
```

Verify the registration:

```sh
test -x "$KNOV_HOST"
node -e '
const fs = require("fs");
const path = process.argv[1];
const manifest = JSON.parse(fs.readFileSync(path, "utf8"));
if (manifest.name !== "com.knov.companion") process.exit(1);
console.log(manifest.path);
console.log(manifest.allowed_origins[0]);
' "$KNOV_MANIFEST_DIR/com.knov.companion.json"
```

Restart Chrome after creating or changing the manifest.

Start the desktop compatibility build so its optional local extension bridge is
available:

```sh
npm run dev:with-extension
```

### 4. Copy the pairing token

Keep the Tauri app running so it creates the database and its protected
per-user Native Messaging socket.

```sh
KNOV_DB="$HOME/Library/Application Support/com.knov.desktop/knov.sqlite3"
sqlite3 "$KNOV_DB" \
  'SELECT pairing_token FROM extension_state WHERE singleton=1;'
```

If that path is absent, locate the database without modifying it:

```sh
find "$HOME/Library/Application Support" \
  -name knov.sqlite3 -print
```

Open the extension's **Details → Extension options**, leave transport set to
**Native Messaging**, paste the token and the approved Chrome profile ID shown
in desktop Settings, save, and choose **Test connection**.
The extension popup should report that it is connected. The desktop Collection
settings continue to show the local database path; extension diagnostics remain
in the extension's own popup because this lane is outside the baseline MVP.

The token is local authentication material. Do not publish it in issues, logs,
screenshots, or source control.

### Development loopback fallback

If Native Messaging registration is unavailable, extension options can use
**Local HTTP (development only)** with:

```text
http://127.0.0.1:48321
```

Use the same pairing token. This binds only to loopback and uses bearer-token
authentication, but it has no TLS and is not the intended release transport.

## Optional companion behavior to verify manually

- Pause desktop collection in the app and confirm the extension stops
  collecting after its next status check. Desktop collection state is
  synchronized to the extension, but only one way: a desktop pause pauses the
  extension, while resuming the desktop does not clear a pause set in the
  extension popup, and pausing the extension does not pause the desktop.
- Add sensitive domains to both the desktop and extension exclusion lists.
  The extension options expose its list; the desktop Rust setting exists, but
  use the desktop and extension exclusion editors for their respective sources.
- Closing the development process stops collection and the local bridge.
- Launch at login is controlled from desktop Settings.

## Reset and uninstall

The in-app **Delete everything** action removes app-owned database rows, resets
settings, attempts to remove both provider Keychain entries, and rotates the
pairing token. It removes Knov's Native Messaging manifest but does not
clear Chrome extension storage.

To remove the manual host registration after stopping Chrome:

```sh
rm "$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.knov.companion.json"
```

This removes only that exact manifest. Remove the unpacked extension from
`chrome://extensions` to clear its local pairing configuration.
See [Privacy model](privacy-model.md#pause-exclusions-and-deletion) for deletion
limits.
