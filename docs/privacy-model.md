# Privacy model

Knoveyla is local-first, not fully local. Raw activity is stored on the Mac, but
profile generation, recommendations, connection tests, and chat use the
user-selected OpenAI or Anthropic API.

## What is collected

The desktop collector can record:

- foreground application name
- focused window title when Accessibility permission is available
- session timestamps and duration
- selected Chrome-history URL, page title, visit time, and recognized search
  query

The Chrome extension can record the focused tab's URL, title, start/end time,
duration, and extension ID. It ignores incognito tabs, non-HTTP(S) URLs, and
locally excluded domains.

Knoveyla does not intentionally collect page bodies, DOM content, form input,
keystrokes, clipboard contents, screenshots, audio, or camera data. The Chrome
extension has no content scripts.

Window titles, page titles, URLs, and search queries can nevertheless contain
sensitive information. Treat the local database as sensitive.

## What remains local

The following remains in app-owned local storage unless the user exports or
copies it outside Knoveyla:

- detailed activity events and dashboard history
- complete imported URLs, titles, and extracted search queries
- generated profile versions, recommendations, and corrections
- settings and Chrome pairing state

Provider keys are stored separately in macOS Keychain. The Chrome pairing token
is stored in SQLite and in the extension's local Chrome storage; it is not a
provider credential.

## What leaves the Mac

| Action | Data sent directly to provider |
| --- | --- |
| OpenAI connection test | API key in authorization; request to list models |
| Anthropic connection test | API key plus a minimal `Reply OK` message |
| Profile refresh | Aggregated activity digest and all authoritative corrections |
| Chat | Structured profile, all authoritative corrections, visible conversation history, and new message |

The profiling digest includes app names, domain-only website identifiers,
durations, counts, and window/page-title strings truncated to 180 characters.
Local redaction removes common credential markers, email-shaped identifiers,
home-directory paths, and long token-like identifiers before truncation. It is
not a general sensitive-data detector, so a title may still disclose private
information.

Knoveyla does not send the complete activity-events table or complete URLs as
part of the profile digest. Chat requests do not attach raw activity events.
Requests go from the Rust core directly to the selected provider; there is no
Knoveyla proxy or analytics service. OpenAI requests set `store: false`.
Provider-side processing and retention remain governed by the selected
provider's API terms and account settings.

## Credentials

The settings and onboarding interfaces pass a newly entered key to a Rust
command, which saves it to Keychain service `com.knoveyla.desktop.llm`. Commands
never return the key to the frontend.

For source development only, `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` in the
native process environment takes precedence over Keychain after a provider has
been configured. Environment variables do not configure first-run provider
selection, and Knoveyla does not load `.env` files automatically. Environment
variables may be visible to other processes with sufficient local privileges
and should not be used for a distributed alpha build.

## Retention

- Normal detailed activity is retained for a rolling 30 days.
- Imported events from days 31–90 are temporary bootstrap data.
- Temporary bootstrap data is deleted only after the first profile refresh
  succeeds. A failed or unavailable provider leaves it in place for retry.
- Profiles and corrections remain until removed through the app's controls.
- The extension does not persist completed activity events. An unfinished active
  session may exist in Chrome session storage.

Expired normal activity is purged while the app is running, including while
collection is paused. If the app is not running, purge execution is delayed
until the next launch.

## Pause, exclusions, and deletion

Desktop collection starts disabled and remains disabled until the user resumes
it from the app. Desktop app exclusions are enforced by the Rust collector and
ingestion core. Extension exclusions and pause state are enforced by the
extension.

The extension checks the desktop collection state before delivery and on its
regular checkpoint. Events completed during a stale-policy window are discarded,
not retained for later upload. Configure domain exclusions in both places when
testing the extension.

`Delete everything`:

- removes app-owned SQLite rows
- resets settings to defaults
- removes both provider credentials from Keychain or reports failure
- creates a new pairing token
- removes Knoveyla's per-user Chrome Native Messaging manifest

It does not promise forensic or cryptographic erasure. SQLite/WAL pages, APFS
snapshots, backups, SSD behavior, crash remnants, and provider-held request data
are outside that guarantee. The database file and Chrome extension storage are
not removed by the in-app action. Clear the
extension's site data or remove the extension to delete its pairing
configuration.
