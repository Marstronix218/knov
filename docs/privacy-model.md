# Privacy model

Knov is a local-first endpoint evidence tool. Detailed computer activity stays in app-owned local storage. The business workflow derives purpose-specific records locally and discloses one only after the user reviews, certifies, previews, and explicitly saves an export.

The alpha is not a compliance service and a certification is a human attestation, not proof of identity, exclusive device use, or statutory compliance.

## Data layers

| Layer | Examples | Default lifetime | Egress rule |
| --- | --- | --- | --- |
| Raw activity | App sessions, window/page titles, URLs, search terms, editor/Git metadata | Rolling 30 days | Stays local; excluded from business exports |
| Derived evidence | Sanitized context, source references, project/category suggestions, confidence, explanations, review overrides | Useful while the draft exists; source detail can expire | Stays local |
| Draft records | Date range, template, projects, categories, totals, review state | Until revised or deleted | Stays local |
| Certified snapshot | Minimal approved totals, period, template, attestation, version, timestamp, hash | Until explicitly deleted | Eligible for explicit export |
| Local audit | Lifecycle action, subject identifier, timestamp | Until deleted with app data | Stays local unless a future explicit minimal export is chosen |

Raw retention must not be evaded by copying URLs, titles, paths, or source rows into derived or certified data. When raw events expire, evidence can retain its review result and minimal derived fields while clearly reporting that detailed provenance is no longer available.

## Collection

The native collector can record:

- foreground application name;
- focused window title when Accessibility permission is granted;
- session timestamps and duration;
- URL, page title, visit time, and recognized search query from an explicitly selected Chrome profile, limited to 30 days;
- metadata-only editor save/history timing and safe relative Git paths.

The optional extension can record focused HTTP(S) tab URL/title timing. It has no content scripts, ignores incognito and non-HTTP(S) pages, and does not persist completed delivery queues.

Knov does not intentionally collect screenshots, pixels, OCR, page bodies, DOM content, form values, keystrokes, clipboard contents, source-file bodies, audio, or camera data. Metadata can still expose sensitive facts, so exclusions and local retention remain essential.

Collection begins under user control. Pause/resume, application exclusions, domain exclusions, browser-profile authorization, and delete-everything controls are enforced in the native ingestion path. Pausing stops new owned activity; retention cleanup can continue.

## Business inference and review

The alpha classifier is deterministic and local. It uses project rules and existing local signals; it makes no LLM call. Suggestions carry a confidence band and explanation and never become user truth merely because they were shown. A user override controls every downstream total.

Reviewing, accepting, correcting, splitting, excluding, or marking evidence personal happens locally. No review action sends data to a third party. Positive-duration evidence must be reviewed or excluded, and included evidence must have a resolved project and category, before certification.

Optional AI-assisted classification is deferred. If added later, it must be separately initiated, use the existing minimized and inspectable context boundary, remove full paths, credential-like strings, and URL query/fragment data, return structured uncertainty, and record that remote inference occurred.

## Certification and export

Certification stores a new immutable, allowlisted snapshot plus a SHA-256 integrity hash. The attestation says the user reviewed the record and believes it reasonably represents the covered work. It does not claim forensic attribution or legal sufficiency. Editing underlying evidence or a record creates a new draft/version and requires a new certification.

The export preview separates **Will be exported** from **Stays private on this Mac**. Default CSV and JSON exports may contain:

- certifying name supplied by the user;
- covered period;
- template name;
- project/client and category allocations;
- hours, percentages, and approved totals;
- certification timestamp and statement;
- record/version and certification identifiers; and
- integrity hash.

Default business exports exclude raw URLs, browsing-history rows, full window/page titles, application timelines, local and repository paths, source event IDs, excluded/personal activity, LLM prompts, and credentials. Native saving uses a local save dialog. Knov has no automatic submission, employer API, raw-database sharing interface, manager mode, or hidden network destination.

## Optional provider and legacy features

OpenAI, Anthropic, and Amazon Bedrock BYOK support remains available only through explicit legacy controls in Settings. Provider credentials are stored in macOS Keychain and never returned to the frontend. The primary evidence, review, certification, and export path does not require a provider and does not perform scheduled provider refreshes.

If a user explicitly invokes a legacy provider feature, the existing minimized-context rules and provider terms apply. That egress is separate from business export and must remain inspectable. Source-development environment overrides may expose credentials to sufficiently privileged local processes and should use limited-purpose keys.

## Retention and deletion

- Raw detailed activity expires after 30 days while Knov runs; a stopped app purges on its next launch.
- Chrome import is optional and limited to that same 30-day window.
- Derived evidence gracefully loses links to expired raw detail.
- Minimal certified snapshots remain until the user deletes them.
- Delete everything removes app-owned rows, resets settings, removes configured Keychain credentials or reports failure, rotates pairing material, and removes Knov's per-user Native Messaging manifest.

Deletion is logical application deletion, not guaranteed forensic erasure. SQLite/WAL pages, APFS snapshots, backups, SSD behavior, crash remnants, exported files, Chrome extension storage, and provider-held data are outside that guarantee. The user controls separately saved exports and must remove the extension or its site data to clear its local pairing state.
