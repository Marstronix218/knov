# Architecture

## Scope

Knov is a single-user, local-first macOS application. React renders the
interface, Rust owns native and security-sensitive operations, SQLite stores
app-owned data, and a Chrome extension supplies accurate active-tab timing.
There is no Knov-hosted backend in the alpha.

## Components

| Component | Location | Responsibility |
| --- | --- | --- |
| React/Vite interface | `apps/desktop/src` | Onboarding, dashboard, history, profile, assistant, and settings |
| Tauri/Rust core | `apps/desktop/src-tauri/src` | IPC commands, collection, Chrome import, retention, SQLite, Keychain, scheduling, and provider calls |
| SQLite store | Tauri application-data directory | Activity, settings, profiles, corrections, recommendations, local inference metrics, and extension pairing state |
| Chrome extension | `apps/extension` | Active-tab URL/title timing, exclusions, pause, and local transport |
| Native Messaging helper | `apps/desktop/src-tauri/src/bin/knov-native-host.rs` | Chrome stdio framing and forwarding to the running Rust core |
| OpenAI, Anthropic, or Amazon Bedrock | external | Profile generation, recommendations, and assistant responses |

No Swift helper is currently used.

## Runtime flow

```text
macOS foreground app/window
            |
            v
      Rust collector ----------------------+
                                            |
selected Chrome History --> temporary copy  |
supported editor metadata ------------------|
                                            v
Chrome tabs --> extension --> local bridge --> SQLite
                                            |
                      aggregate/redact/domain-only digest
                                            |
                                            v
                           selected BYOK AI provider
                                            |
                                            v
                             local profile/recommendations
```

The frontend calls typed Tauri commands through `invoke`. It does not open the
database, read Keychain, or call providers directly. Outside Tauri, the same
frontend returns explicit mock data for design and browser tests.

## Collection

The Rust collector samples every five seconds by default. On macOS it invokes
`System Events` through `osascript` to identify the frontmost application and,
when Accessibility permission permits it, the front-window title. A continuous
session is stored when the app or title changes.

Chrome history import:

1. Discovers profiles under
   `~/Library/Application Support/Google/Chrome`.
2. Requires the user to select at least one profile.
3. Copies each selected `History` database to a uniquely named temporary file.
4. Reads visits from the previous 90 days.
5. Deletes the temporary copy after the import attempt.

Visits older than 30 days are flagged as temporary bootstrap data. They remain
until the first profile succeeds, then are deleted.

Supported editors contribute metadata-only Local History save signals. When
those indexes or Accessibility window titles are unavailable, Knov can derive
recent safe relative paths from Git metadata in the most recently active
workspace. Source contents, Local History snapshots, hidden files, generated
trees, dependencies, and credential-like paths are not opened or stored.

The extension observes the active HTTP(S) tab while Chrome is focused. It stores
only the unfinished session in `chrome.storage.session`. Completed events receive
one delivery attempt and are not persisted or retried. The extension does not
use content scripts. Each installation is configured with an approved native
Chrome profile ID, which the ingestion core enforces.

## Local extension transports

Native Messaging is the intended transport. Chrome starts
`com.knov.companion`, which forwards framed messages over a mode-0600 Unix
domain socket in the app-data directory. The Rust core validates protocol
version, pairing token, and extension ID before accepting events.

A loopback HTTP transport exists for development. It accepts only loopback HTTP
endpoints and requires a bearer pairing token. It is not the production
transport and does not provide TLS.

Source builds require loading the extension unpacked and building the helper.
Settings exposes host registration and the pairing token. See
[Alpha setup](alpha-setup.md).

## Storage

The Rust core is the only SQLite writer. On macOS, Tauri resolves the database
under its application-data directory, normally:

```text
~/Library/Application Support/com.knov.desktop/knov.sqlite3
```

SQLite runs in WAL mode. The schema uses `PRAGMA user_version` migrations.
Provider keys are not stored in SQLite; Keychain entries use service
`com.knov.desktop.llm` and provider account names `openai`, `anthropic`, or
`bedrock`.

Main stored records:

- detailed app, window, URL, page-title, and search-query events
- selected Chrome profiles and collection settings
- generated profile versions and recommendations
- separately stored authoritative user corrections
- pairing token, first authenticated extension ID, and last-seen timestamp
- local context-economics records for completed assistant queries

Chat messages are held in frontend memory for the current session and are not
persisted by Knov.

## Profiling and scheduling

Profile refresh produces a local aggregate containing at most 200 grouped
activity entries. Each entry includes app name, a truncated locally redacted
title, domain only, accumulated seconds, and occurrence count.
The digest and authoritative corrections go directly to the selected provider.

Assistant queries use a separate context path. Knov retrieves relevant profile
facts locally, computes compact query-specific activity facts, sanitizes an
explicitly selected thread packet, and deterministically packs the highest-value
units under a token budget. A larger comparison prompt is measured locally but
is never sent. Amazon Bedrock additionally performs model-specific `CountTokens`
preflight and uses prompt-prefix caching when eligible.

The scheduler checks once per minute and attempts one refresh per local calendar
day when a provider and credential are available. This also provides catch-up
after sleep or restart. Manual refresh uses the same provider path. A successful
first refresh deletes bootstrap activity older than 30 days.

## Current implementation boundaries

- Chrome is implemented; Safari and Firefox are not.
- Native helper registration is not a polished installer flow.
- Extension exclusions are stored separately; the desktop collection state is
  synchronized on extension status checks.
- Launch at login is persisted locally and applied through the Tauri autostart
  plugin.
- Behavioral guidance is suppressed during generation and dashboard display
  when disabled.
- Provider-key removal is available in Settings.
- Profile summary editing, inferred-item suppression, and editable authoritative
  corrections are available locally.
- The dashboard derives top-application, longest-session, distinct-page, and
  cautious local topic/category insights; provider recommendations are also
  implemented.
- The bundled frontend uses a restrictive Tauri content security policy.
