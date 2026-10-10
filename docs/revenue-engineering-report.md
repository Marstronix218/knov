# Revenue implementation report

## Implemented

A new top-level Revenue destination integrates a persistent commercial review
loop into the existing desktop app. Its isolated demo includes Acme's third
dashboard, Northstar's overdue proposal, Atlas's uncertain billing, and an
already-approved negative case.

- `apps/desktop/src-tauri/src/revenue.rs`: clients, projects, agreements,
  commitments, evidence, opportunities, corrections, drafts, outcomes, metrics,
  and project-specific commercial procedure suggestions in local SQLite.
  Deterministic rules deduplicate candidates, require source evidence, preserve
  unknown amounts, and keep missing billing/approval information provisional.
- `apps/desktop/src-tauri/src/discovery.rs`: optional commercial workflow context
  and brief clarification using the existing session, revision, workflow, and
  graph persistence. Demo sessions are excluded from actual workflow retrieval.
- `apps/desktop/src-tauri/src/revenue/connectors.rs`: scoped Gmail and Slack
  read-only adapters, Keychain credentials, authorization validation,
  disconnect/revoke, race-safe deletion, bounded atomic sync, and local document
  extraction.
- `apps/desktop/src/pages/RevenuePage.tsx`, `revenue.css`, `src/revenueTypes.ts`,
  `src/lib/revenueApi.ts`: overview, inbox filters, evidence details, targeted
  clarification, project/import/source controls, source classification,
  editable drafts, approval, recorded outcomes, and workflow stage states.
- `src-tauri/src/db.rs`, `lib.rs`, `commands.rs`, `src/App.tsx`, `src/types.ts`:
  schema initialization, retention/deletion, command registration, navigation,
  and shared workflow typing. README and DESIGN.md describe the extension.
- Revenue Rust tests and `src/revenue.test.tsx`, plus an App navigation regression.

No required hosted backend or package dependency was added. Existing uncommitted
work was retained; no commit or deployment was made.

## Reused and integration boundaries

The existing Tauri/Rust/SQLite architecture, React components/design tokens,
workflow interview storage/revisions/graphs, secure credential library, and
privacy/deletion infrastructure are reused. Projects link existing work-thread
IDs deliberately. User-confirmed commercial context is additional workflow
memory, not a replacement for general interviews. Recurring confirmations
suggest a procedure for review in the existing Workflows/Skill surface; they do
not create a second runtime or enable financial execution.

Revenue analysis and clarification run locally and deterministically. The
existing BYOK provider and general AI interviewer remain available unchanged;
this slice does not send contracts or communications to an external model or
introduce a separate sales chatbot.

## Functional paths and proof

The desktop commands and UI support project creation, agreement import and
correction, reviewed work/communication evidence, detection, contextual answers,
draft preparation/editing/approval, and separately recorded outcomes. The full
seeded backend loop is tested through database reopen, and component tests cover
review interactions and data-scope switching. Preparing or approving a draft
never sends an email, posts a message, issues an invoice, or records payment.

Gmail/Slack are real API adapters. Their account authorization, actual Keychain
operations, provider responses, and remote revocation have **not** been tested
with live credentials. Demo fixtures are explicit, isolated records, not live
sync results. Native visual smoke testing was unavailable because the computer
use tool could not start its app-server; there is no screenshot verification.

## Run and configure

```sh
npm install
npm run dev:desktop
```

Complete setup, open Revenue, choose **Open isolated demo**, then **Load
reproducible demo**. Select Acme, ask the clarification, choose separately
billable, prepare/edit/save/approve the change-order draft, then record an actual
outcome separately. Return to actual data for client creation and real imports.

See [Revenue intelligence](revenue-intelligence.md) for evidence/retention
semantics and [source setup](revenue-connectors.md) for exact OAuth credentials,
scopes, selected-source ranges, token expiry, and disconnect behavior.

## Validation

The implementation was checked with the repository's required commands:

| Command | Result |
| --- | --- |
| `npm run typecheck --workspace @knov/desktop` | Passed |
| `npm test --workspace @knov/desktop` | 113 tests passed |
| `npm run build --workspace @knov/desktop` | Passed |
| `npm run check:rust` | Passed |
| `npm run test:rust` | 178 tests passed |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --no-deps -- -D warnings` | Passed |
| `git diff --check` | Passed |

Independent review found and verified fixes for false completion/scope
resolution, conflicting classifications, demo scope leakage, deletion/sync
races, corrupted-record replacement, and Unicode partial imports. Tests cover
future/negated language, immutable correction provenance, approved negative
cases, explicit overdue dates, unknown billing, source restrictions, retention,
financial separation, and the original discovery/thread regressions.

## Remaining limitations and next step

Detection is intentionally conservative and does not parse arbitrary contracts.
Synced communications require user-reviewed classification. Scope matching
covers the dashboard example and simple exclusions; complex contractual terms
remain a human decision. Monetary amounts are explicit integer cents in USD,
EUR, GBP, CAD, and AUD. Receipt checks are user-attested with user-confirmed
provenance, not independent accounting verification or proof of Knov causation.
Multiple partial payments/refunds are not reconciled.

Gmail refresh-token/background sync and embedded Slack OAuth are not implemented.
Slack uses externally authorized read-only token provisioning. Text PDFs need
Poppler; the development host lacked it, and OCR is unsupported. Connector
coverage may be truncated and is never represented as complete financial data.

The smallest next engineering step is one explicitly authorized invoice source
with coverage dates and receipt reconciliation, validated with an agency's
reviewed project records.
