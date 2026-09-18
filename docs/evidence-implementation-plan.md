# Endpoint evidence pivot implementation plan

Baseline before edits: TypeScript check passed; 59 frontend tests and 76 Rust tests passed.
Repository instructions were supplied in the task; no repository AGENTS.md exists.

1. Extend the existing SQLite migration array and native Database boundary with projects,
   derived evidence, draft records, immutable certifications, and content-minimal audit events.
   Reuse ActivityEvent, retention/exclusions, metadata sanitization and semantic threads.
2. Put deterministic classification, overlap-safe duration accounting, aggregation,
   snapshot serialization, SHA-256, and CSV/JSON allowlisting in a Rust business module.
   Only explicit certification creates a snapshot. Exports render the stored snapshot.
3. Add a typed frontend business API and clearly synthetic browser-preview implementation.
   Build Evidence, Projects, Records / Review & Certify, and Exports with native-backed actions.
4. Integrate the new navigation and local-only onboarding; retain existing activity/privacy
   settings and move personal memory functionality to a secondary legacy surface.
   Remove automatic provider scheduling from normal app operation.
5. Update product, architecture, privacy, threat, setup, and testing documentation.
6. Verify targeted model/privacy tests, TypeScript, frontend tests/build, Rust tests/checks,
   formatting, packaged build, and browser interactions. Record native-only validation gaps.

Decisions: Unix-second half-open periods; confidence bands rather than precise probabilities;
deterministic classification requires no provider; R&D categories are provisional and experimental;
history visits/editor metadata do not manufacture foreground duration; excluded and unallocated
time stay visible. Certified versions survive raw retention and are never recomputed on export.
