# Architecture

## Scope and trust boundary

Knov is a single-user, local-first macOS endpoint-evidence application. React presents the workflow, Rust owns native and security-sensitive operations, and SQLite is the durable store. There is no Knov-hosted backend, account system, employer portal, or automatic submission service.

The primary path is:

```text
macOS / selected browser / editor metadata
                    ↓
             raw activity events
                    ↓
       local deterministic classification
                    ↓
          derived evidence with provenance
                    ↓
        authoritative human review
                    ↓
             versioned draft record
                    ↓
      immutable minimal certification snapshot
                    ↓
        previewed local CSV or JSON export
```

## Components

| Component | Location | Responsibility |
| --- | --- | --- |
| React/Vite interface | `apps/desktop/src` | Evidence, Projects, Records, Review & Certify, Exports, Activity, onboarding, and Settings |
| Shared business contract | `apps/desktop/src/businessTypes.ts` | Typed project, evidence, record, certification, audit, and export boundary |
| Tauri/Rust core | `apps/desktop/src-tauri/src` | IPC, collection, classification, aggregation, certification, export, retention, SQLite, Keychain, and local audit |
| SQLite | Tauri application-data directory | Raw events, projects, evidence, record versions, certifications, audit entries, settings, exclusions, and legacy data |
| Optional Chrome extension | `apps/extension` | Experimental active-tab metadata and local transport |
| Optional Native Messaging helper | `apps/desktop/src-tauri/src/bin/knov-native-host.rs` | Chrome stdio framing and forwarding to the local Rust core |
| Optional BYOK providers | external | Legacy personal-context features only; not the alpha business-classification path |

Outside Tauri, the frontend uses explicitly synthetic data for design and browser testing. Browser preview export is synthetic; a native build uses the operating-system save dialog.

## Collection and evidence derivation

The collector records foreground application sessions and, with Accessibility permission, active-window titles. A user may explicitly select Chrome profiles for a local import limited to the last 30 days. Selecting or importing a profile does not invoke an LLM. Editor signals remain metadata-only: safe relative paths and save/history timing may support classification, but file bodies and Local History snapshots are not opened.

The optional extension observes active HTTP(S) tab metadata while Chrome is focused. It has no content scripts and ignores incognito and non-HTTP(S) pages. Native Messaging is the intended local transport; the authenticated loopback transport remains development-only.

Evidence is derived locally from retained activity. Each item separates:

- observed time, source type, application, and local source-event provenance;
- sanitized display context;
- inferred project/category, confidence band, method, and explanation; and
- authoritative user status and overrides.

Deterministic classification considers confirmed mappings, repositories and paths, domains, aliases and keywords, known legacy thread signals, and temporal continuity. Confidence uses `high`, `medium`, or `low`; low-confidence evidence remains unallocated. Optional AI classification is deferred, and the primary business path makes no provider request.

Time-bearing evidence uses Unix-second half-open ranges `[start, end)`. Overlapping intervals are deduplicated before aggregation. Chrome history rows and editor/save metadata can corroborate a classification but have zero allocatable duration unless paired with observed foreground activity.

## Storage and migration

The Rust core is the only SQLite writer. On macOS the database is normally:

```text
~/Library/Application Support/com.knov.desktop/knov.sqlite3
```

SQLite uses WAL mode and ordered `PRAGMA user_version` migrations. Schema version 6 adds durable projects, evidence, record versions, certifications, and content-minimal audit entries while retaining activity, exclusions, selected browser profiles, corrections, provider settings, and other legacy rows.

Raw source events are retained for 30 days. Derived evidence may show that its source detail has expired. Certified snapshots survive raw retention because they contain only the approved derived record fields. Certifications remain until explicitly deleted; they must not embed raw URLs, full titles, source paths, or activity rows.

Provider keys remain in macOS Keychain under service `com.knov.desktop.llm`, never in SQLite or frontend responses.

## Records and certification

Record templates define reusable dimensions, categories, required fields, and export columns. The alpha includes Generic Project Allocation, R&D Allocation — Demo / Experimental, and Professional Services Allocation.

Draft aggregation reports tracked, reviewed, unreviewed, excluded, and unallocated time. A record is certifiable only when every positive-duration item in scope has been reviewed or excluded and every included item has a resolved project and category. Zero-duration history/editor signals do not block certification.

Certification requires the explicit attestation action. Rust canonically serializes an allowlisted snapshot, stores it as a new immutable certification, and computes SHA-256 over that serialization. A later edit creates a new record version and supersedes the working draft; the earlier certified version and hash do not change.

CSV and JSON are rendered from the stored certification snapshot, not reconstructed from mutable evidence. Export preview and saved output share the same allowlist. Native export presents a save dialog. There is no network export path.

## Audit and legacy surfaces

The local audit trail records lifecycle actions such as project changes, review decisions, exclusions, record generation, certification, supersession, and export. Entries contain identifiers, action names, and timestamps rather than sensitive source content.

The older Now, Threads, Memory, assistant, profile/recommendation, and BYOK facilities are secondary legacy context under Settings. They may reuse retained data, but they do not compete with the primary navigation or trigger scheduled provider refreshes. Explicit legacy provider controls remain available.

## Current alpha boundaries

- The tested native target is Apple Silicon macOS 26.
- Accessibility permission is optional and affects window-title evidence.
- Chrome history import is optional; Safari and Firefox collection are not release targets.
- The Chrome extension and its Native Messaging setup remain an experimental compatibility lane.
- Exports are local files only; there is no employer integration or network destination.
- The R&D template demonstrates the architecture and carries no compliance guarantee.
- Code signing, notarization, updates, multi-user identity proof, and independent security assessment remain outside this source alpha.
