# Alpha setup

## Supported environment

The tested native target is an Apple Silicon Mac running macOS 26. This source alpha is unsigned and not notarized.

Install Xcode Command Line Tools, Node.js 20.19+ or 22.12+, npm, and the current stable Rust toolchain. Chrome is optional and needed only for selected-profile import or extension testing.

```sh
node --version
npm --version
rustc --version
cargo --version
xcode-select -p
uname -m
```

`uname -m` should print `arm64`.

## Run the native app

```sh
npm install
npm run dev:desktop
```

The first launch creates the SQLite database in the Tauri application-data directory. Onboarding explains that Knov records limited metadata locally, prepares draft work records, requires human review, and shares nothing until explicit export. It does not require an AI provider, API key, Chrome profile, or extension.

Create a project after onboarding, then resume collection when ready. The primary navigation is **Evidence**, **Projects**, **Records**, **Review & Certify**, **Exports**, **Activity**, and **Settings**. Earlier personal-context and provider features remain secondary in Settings.

### macOS Accessibility

Window-title evidence requires permission at **System Settings → Privacy & Security → Accessibility**. Without it, foreground app duration can still be recorded, but classification has less context. Restart the Tauri process if macOS does not apply a permission change immediately.

Accessibility permission does not add screenshot, keystroke, clipboard, page-body, source-body, microphone, or camera capture.

### Optional Chrome history

In Settings, explicitly select only the Chrome profiles you want Knov to inspect. Import is local and limited to the previous 30 days. It does not require an AI provider and does not make an LLM call. Chrome visit rows provide contextual provenance; they do not by themselves establish foreground duration.

## Synthetic browser preview

```sh
npm run dev
```

Browser preview uses clearly marked fake data and cannot collect activity or invoke native commands. Follow this exact demo path:

1. In **Evidence**, compare a high-confidence suggestion with ambiguous and unallocated items.
2. In **Projects**, inspect Search Ranking V2, Authentication, and Internal Operations.
3. In **Evidence**, accept, correct, or exclude every positive-duration item and resolve project/category fields.
4. In **Records**, generate an **R&D Allocation — Demo / Experimental** draft with all three projects and inspect its totals.
5. In **Review & Certify**, accept the attestation, click **Preview certification**, certify the record, then inspect its immutable version and hash.
6. In **Exports**, compare the outbound allowlist with the raw fields that stay private, then preview CSV and JSON.
7. Revise the record and confirm certification now requires a new version while the old certification remains unchanged.

Synthetic browser export does not prove native file saving. Use the native app to test the operating-system save dialog.

## Native evidence demo

1. Create a project with aliases, keywords, domains, repositories, or path rules.
2. Resume collection and work across a matching app/window or selected browser profile.
3. Inspect the derived evidence. Check its observed facts, inferred fields, confidence, explanation, and provenance.
4. Correct suggestions where needed. User choices are authoritative.
5. Generate a record for a bounded period. Ranges are `[start, end)` and overlapping foreground intervals are deduplicated.
6. Resolve every positive-duration item or exclude it, then certify with the explicit attestation.
7. Save CSV or JSON through the native dialog. No network upload occurs.
8. Revise the record and verify that the certified snapshot and hash remain unchanged while the revision becomes a new draft/version.

The R&D template is an experimental architecture demonstration. It is not tax, legal, accounting, employment, or compliance advice.

## Optional legacy BYOK controls

OpenAI, Anthropic, and Amazon Bedrock settings remain available for the secondary legacy personal-context experiment. Keys are stored in macOS Keychain. They are not used for deterministic project/category classification, record generation, certification, or export, and the primary workflow does not schedule provider refreshes.

For source development, an explicitly configured legacy provider can be overridden by `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or `AWS_BEDROCK_API_KEY` in the native process environment. Knov does not load `.env` files automatically. Use limited-purpose keys and never commit them.

## Optional Chrome extension setup

The extension is an experimental compatibility lane for active-tab timing. Baseline onboarding and native evidence collection do not depend on it.

Build and load it:

```sh
npm run build --workspace @knov/chrome-extension
cargo build \
  --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --bin knov-native-host
```

Open `chrome://extensions`, enable Developer mode, choose **Load unpacked**, select `apps/extension/dist`, and copy its 32-character extension ID.

Register the exact helper path in Chrome's per-user Native Messaging directory:

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

Restart Chrome, run `npm run dev:with-extension`, and pair the extension with the token and approved profile ID exposed by the development settings. Treat the token as local authentication material and do not publish it.

Native Messaging is the intended transport. The `http://127.0.0.1:48321` bearer-token fallback is development-only, has no TLS, and expands local attack surface. Extension exclusions remain source-specific; configure them in the extension as well as in the native app.

## Local data and reset

The database is normally:

```text
~/Library/Application Support/com.knov.desktop/knov.sqlite3
```

Raw activity expires after 30 days. Certified minimal snapshots remain until deleted. **Delete everything** removes app-owned rows and certifications, resets settings, attempts to remove provider Keychain entries, rotates pairing material, and removes Knov's Native Messaging manifest. It does not securely erase SQLite/WAL pages, backups, already exported files, or Chrome extension storage.

To remove the extension lane, remove the unpacked extension and delete only its exact manifest after Chrome stops:

```sh
rm "$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.knov.companion.json"
```
