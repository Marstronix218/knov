# Knov

Knov is a local-first endpoint evidence tool that turns limited computer-activity metadata into user-reviewed, user-certified business records. Raw activity stays on the user's device; only a record the user explicitly approves is exported.

This repository contains a technical alpha and R&D demonstration. The included record templates, including R&D Allocation, are examples only. Knov does not provide tax, legal, accounting, employment, or regulatory advice and does not guarantee that a record satisfies any compliance requirement.

## The workflow

```text
Activity captured on this Mac
          ↓
Locally derived evidence
          ↓
Project and category suggestions
          ↓
User review and correction
          ↓
Immutable certified snapshot
          ↓
Explicit CSV or JSON export
```

The computer can observe enough metadata to help prepare a record without giving an employer, client, accountant, or auditor the underlying browsing and application history. Knov has no employer interface, manager mode, hosted account, or background export endpoint.

## Alpha capabilities

- Native Apple Silicon macOS app built with Tauri 2, React, TypeScript, Rust, and SQLite
- Foreground application and active-window-title collection
- Optional, explicitly selected Chrome-profile history import with a 30-day limit
- Optional experimental Chrome extension for active-tab timing
- Metadata-only editor Local History and Git-path signals
- Local deterministic project and category suggestions with confidence bands and explanations
- Projects with aliases, keywords, domains, repositories, and path signals
- Evidence review, correction, exclusion, splitting, and high-confidence bulk acceptance
- Generic Project, experimental R&D, and Professional Services record templates
- Immutable certified snapshots with record versions and SHA-256 integrity hashes
- Previewed, allowlisted CSV and JSON exports
- Local audit history for the record lifecycle
- Pause, exclusions, rolling retention, and deletion controls
- Legacy personal-context and BYOK provider controls under Settings

Knov deliberately does not capture screenshots, page bodies, DOM content, form values, keystrokes, clipboard contents, microphone audio, or camera data. Titles, URLs, and paths can still be sensitive, so detailed activity remains local and expires after 30 days.

## Run the alpha

Requirements:

- Apple Silicon Mac running macOS 26 (the tested native target)
- Xcode Command Line Tools
- Node.js 20.19+ or 22.12+ and npm
- Current stable Rust toolchain
- Google Chrome only if you choose Chrome history import or the optional extension

```sh
git clone https://github.com/Marstronix218/knov.git
cd knov
npm install
npm run dev:desktop
```

The native app stores its SQLite database in the Tauri application-data directory. Accessibility permission is optional: app-duration collection works without it, while window-title evidence requires it.

Onboarding is local and does not require an AI provider, API key, Chrome profile, or extension. It explains the collection boundary and then opens the evidence workspace. You can create a project immediately or use the synthetic browser preview first.

### Synthetic browser demo

```sh
npm run dev
```

This opens an explicitly synthetic workspace with example projects, reviewed and ambiguous evidence, excluded personal activity, and a draft R&D record. It cannot collect real activity, write native exports, access Keychain, or prove native SQLite behavior.

Suggested demo:

1. Open **Evidence** and inspect a high-confidence suggestion and its explanation.
2. Correct or accept every positive-duration item, resolve each project/category, and leave the seeded personal block excluded.
3. Open **Records**, generate an experimental R&D Allocation draft that includes the three sample projects.
4. Open **Review & Certify**, enter an attestor name, accept the statement, preview the proposed certification fields, and certify the ready record.
5. Open **Exports**, compare **Will be exported** with **Stays private on this Mac**, then preview CSV or JSON.
6. Revise the record. The prior certification remains unchanged and the revision requires a new certification.

### Real native demo

1. Run `npm run dev:desktop` and complete the local onboarding.
2. Create projects and their matching signals.
3. Resume collection; optionally grant Accessibility for window titles.
4. Optionally select a Chrome profile. Knov imports at most 30 days of history locally; selection does not trigger an LLM call.
5. Review evidence, resolve all positive-duration work, generate a record, and certify it.
6. Export the certified snapshot. Native builds use a save dialog; no network upload occurs.

For complete source setup, optional Chrome-extension instructions, and known limitations, see [Alpha setup](docs/alpha-setup.md).

## Record rules

- The UI's **Through** date is inclusive. Internally, record ranges are stored as half-open `[start, end)` intervals to make clipping and aggregation deterministic.
- Overlapping observed intervals are deduplicated rather than double-counted.
- Chrome history visits and editor saves are useful provenance but do not manufacture foreground duration.
- A record cannot be certified while positive-duration evidence is neither reviewed nor excluded, or while included evidence has an unresolved project/category.
- User corrections are authoritative.
- Certification stores a minimal derived snapshot, never a hidden copy of raw URLs, titles, paths, or source events.
- Editing after certification creates a new draft/version; it never changes the certified snapshot.
- Raw activity is retained for 30 days. Derived detail may lose provenance as raw events expire. Minimal certifications remain until the user deletes them.

## Development checks

```sh
npm run typecheck --workspace @knov/desktop
npm test --workspace @knov/desktop
npm run build --workspace @knov/desktop
npm run check:rust
npm run test:rust
```

See [Testing](docs/testing.md), [Architecture](docs/architecture.md), [Privacy model](docs/privacy-model.md), and [Threat model](docs/threat-model.md) for the contracts behind these checks.

## Optional legacy context and AI controls

The earlier personal-memory experiment remains secondary under Settings for compatibility. OpenAI, Anthropic, and Amazon Bedrock BYOK controls are optional and are not used by the endpoint-evidence business workflow. The business classifier is deterministic and local in this alpha; optional AI classification is deferred. No scheduled provider refresh runs in the primary workflow.
