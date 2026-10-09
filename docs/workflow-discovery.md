# Workflow intelligence: implementation slice

## Repository audit

Knov already uses Tauri 2, React, TypeScript, Rust, and SQLite. Its collectors,
semantic threads, provider clients, Keychain credential handling, context
sanitizer, and agent runtime are extension points, not replacement targets.

The existing `agent/` modules mine repeated activity, rank opportunities, store
editable Skills, enforce permissions, and execute and verify bounded actions.
Those features remain independent of interview-derived workflow documents.
An interview must not silently authorize an action or turn a proposed process
into an executable Skill.

The new slice completes the interview-to-workflow and workflow-knowledge-graph
milestones first. Gmail/Slack OAuth and synchronization, communication linking,
interview-derived opportunity ranking, and interview-to-Skill generation remain
future milestones. Existing activity-derived opportunities and Skills continue
to work.

## Baseline verification

Before implementation, the desktop frontend suite passed 83 tests in four
files and the native suite passed 125 tests. The working tree was clean.

## Privacy boundaries

Interview answers and workflow revisions are sensitive user-authored content.
They remain in the existing local database. Requesting AI analysis sends a
bounded interview representation and minimized selected-thread context directly
to the configured BYOK provider. Users must review the disclosure before using
this feature. Raw activity tables are not sent, and provider credentials remain
in Keychain.

Activity metadata is evidence of application or resource use. Semantic thread
groupings and model-authored process descriptions are hypotheses about intent.
Only explicit user review confirms a workflow; a model cannot grant execution
permissions. Provider output is parsed and validated as data and never becomes
shell commands or agent tool instructions.

## Running the application

Use `npm install`, then `npm run dev:desktop`. Configure a provider and save its
key in Settings for AI analysis. Existing development credential overrides
(`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `AWS_BEDROCK_API_KEY`) remain supported;
there are no new required environment variables. Browser preview cannot access
the native database or credentials.

## User experience

Open **Workflow Discovery** to start from a description or a selected existing
work thread. Starting saves a local session and its first question. Answering
or skipping uses the configured provider; pausing and finishing preserve the
current reconstruction locally. Saved sessions resume after restarting Knov.

Review the workflow document, edit its fields and ordered steps, and confirm it
only when its conclusions match your work. Saving a draft is distinct from
confirmation. **Knowledge → My workflows** displays interview-derived documents
with a step diagram and exposes their nodes, relationships, and provenance.
Activity-derived Workflows and Skills retain their existing experience.

## Storage and schema

The existing database initializes additive, idempotent discovery tables:
`discovery_sessions`, `discovered_workflows`, `discovery_graph_revisions`,
`discovery_graph_nodes`, `discovery_graph_edges`, and
`discovery_graph_evidence`. No separate graph service or database is required.
Session documents and their graph snapshots are written in one transaction.
Nodes and edges are indexed for source, target, kind, and evidence lookup.
Updates append graph snapshots, preserving previous claims for review.

Session revision checks reject stale AI responses after a pause, correction,
or completion. Explicit user edits remain authoritative over future synthesis.
Model confidence and estimated duration remain estimates. Workflow documents
are validated natively for bounded size, unique steps, supported evidence
types, finite confidence, and dependencies on preceding steps.

## Major implementation files

| File | Responsibility |
| --- | --- |
| `apps/desktop/src-tauri/src/discovery.rs` | Interview lifecycle, validated documents, adaptive synthesis, authoritative corrections, and persistence |
| `apps/desktop/src-tauri/src/discovery/graph.rs` | Relational snapshots, provenance, historical queries, and deletion |
| `apps/desktop/src-tauri/src/db.rs` | Additive schema initialization and global deletion |
| `apps/desktop/src-tauri/src/commands.rs`, `lib.rs` | Typed discovery IPC and command registration |
| `apps/desktop/src-tauri/src/providers.rs` | Reuses existing completion and structured JSON parsing |
| `apps/desktop/src/pages/DiscoveryPage.tsx` | Saved conversational interviews and controls |
| `apps/desktop/src/pages/WorkflowEditor.tsx` | Workflow correction, confirmation, and step diagram |
| `apps/desktop/src/pages/KnowledgePage.tsx` | Workflow list, graph evidence, and revision inspection |
| `apps/desktop/src/pages/discovery.css` | Existing design tokens and responsive discovery layout |
| `apps/desktop/src/App.tsx`, `types.ts`, `lib/api.ts` | Navigation, typed contracts, and native API calls |

## Validation boundaries

Synthetic native fixtures exercise structured provider-response processing,
state resumption, corrections, provenance, graph updates, and deletion without
contacting an external provider. Frontend component tests use explicit API
fixtures. These tests do not demonstrate real OAuth authorization or a live
provider conversation. The full communication-to-automation end-to-end scenario
belongs to the later connector and Skill milestones.

The repository has no desktop ESLint script; TypeScript checking, production
frontend builds, Rust checks, strict Clippy, and test suites provide the current
automated checks. Browser automation was unavailable in this environment, so
interactive visual inspection and a packaged native conversation remain manual
validation steps.

Final integrated checks for this slice:

- `npm test`: 93 desktop tests passed across six files.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --quiet`:
  144 native tests passed, including reopening the database after an interview.
- `npm run build`: TypeScript checking and Vite production build passed.
- `npm run check:rust`: passed.
- `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --lib -- -D warnings`:
  passed.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`
  and `git diff --check`: passed.
- `npm run test:extension`: 15 existing extension tests passed;
  `npm run typecheck:extension`: passed.

## Next engineering milestones

1. Implement minimum-scope Gmail and selective-channel Slack connectors, with
   OS credential storage, incremental sync, rate limits, and cache deletion.
2. Link minimized communication evidence to workflow revisions before adding
   interview-derived opportunity scores. Label time savings as estimates.
3. Generate versioned Skills only from user-confirmed workflows and approved
   opportunities, using the existing bounded adapters and action journal.
4. Evaluate retrieval-based personalization using explicit correction and
   execution histories; keep source content separate from agent instructions.

Live provider and OAuth validation require user credentials. Synthetic tests
must not be reported as live authorization or successful remote execution.
