# Knov — Product Requirements

**Privacy-preserving endpoint evidence for user-certified business records**

Version 1.0 — Technical alpha / R&D demonstration

## 1. Product definition

Knov turns limited computer-activity metadata into reviewable, purpose-specific business records. Detailed activity stays on the user's Mac. A third party receives only a minimal record that the user has corrected, explicitly certified, previewed, and exported.

```text
Actual computer activity
    → local capture
    → derived evidence
    → project/category suggestion
    → draft record
    → human review
    → immutable certification
    → constrained export
```

The core promise is: **the computer may observe enough activity to derive the record, but a third party should not automatically receive everything the computer observed.**

This technical alpha demonstrates the architecture. Example templates, especially R&D Allocation, do not constitute tax, legal, accounting, employment, or compliance advice and carry no guarantee of regulatory sufficiency.

## 2. Users and jobs

The primary user is an individual knowledge worker who must prepare a defensible allocation or activity summary for an accountant, client, auditor, or other third party without surrendering raw endpoint history.

Core jobs:

1. Define projects or business components and their recognizable local signals.
2. See what limited evidence Knov derived and why it suggested an allocation.
3. Correct, split, exclude, or mark uncertain evidence locally.
4. Build a record whose reviewed, unreviewed, excluded, and unallocated time remains visible.
5. Attest to a fixed record version.
6. Inspect the exact outbound fields and save a purpose-specific file.

The alpha is single-user and local. It does not prove who physically used a device, replace professional judgment, or provide a multi-user approval system.

## 3. Product boundaries

Knov must not include an employer portal, manager mode, employee/productivity score, background third-party submission, raw-database sharing, or network streaming of activity. It must never silently expose URLs, browsing history, window titles, app timelines, screenshots, paths, or source events to an employer or other recipient.

The metadata-first collection boundary excludes screenshots, OCR, page bodies, DOM content, form values, keystrokes, clipboard contents, source-file bodies, microphone/audio, and camera capture.

Existing foreground app/window collection, selected Chrome history, optional extension timing, editor/Git metadata, local SQLite, exclusions, pause/delete, provenance, corrections, thread signals, and provider infrastructure should be reused. Personal-memory and assistant features may remain only as a secondary legacy context surface in Settings.

## 4. Primary experience

The primary navigation is:

1. **Evidence** — derived evidence queue and source/provenance inspection
2. **Projects** — work-unit definitions and matching rules
3. **Records** — templates, ranges, drafts, versions, and totals
4. **Review & Certify** — local resolution and explicit attestation
5. **Exports** — outbound preview and local CSV/JSON saving
6. **Activity** — raw local timeline
7. **Settings** — collection, privacy, sources, deletion, and legacy context/provider controls

The product introduction is: “Knov turns your local work activity into reviewable, purpose-specific business records without exposing your raw activity.” The UI must describe suggestions as inference, corrections as user decisions, and certification as a human attestation.

## 5. Data model

The TypeScript business contract lives in `apps/desktop/src/businessTypes.ts`; Rust remains authoritative for persistence and security-sensitive rules.

### Project

A durable project/work unit contains an ID, name, description, display color, active/archived status, aliases, keywords, domains, repositories, paths, optional client and external identifier, and timestamps. Users can create, edit, archive, safely delete, and manually associate evidence. Existing inferred thread signals may inform suggestions but never override explicit project rules or user corrections.

### Evidence item

Evidence references one or more retained source events and contains a half-open `[start, end)` interval, duration, source types, application, sanitized context, suggested project/category, confidence bands, explanation, inference method, review state, user overrides, source availability, and timestamps.

Observed facts, inferred fields, and user decisions must be distinct. User overrides are authoritative. Missing/expired sources are visible. Raw activity rows are never themselves certified business records.

### Record template

Templates define reusable dimensions, categories, required fields, and export columns. The alpha provides:

- **Generic Project Allocation** — project, hours, percentage, and review status.
- **R&D Allocation — Demo / Experimental** — user-defined business components with Direct Research, Direct Supervision, Direct Support, Non-R&D / Unqualified, and Unallocated.
- **Professional Services Allocation** — client, project/matter, billable/non-billable, and hours.

### Record and certification

A record stores a template, half-open covered period, selected projects, totals, version, timestamps, and state: Draft, Needs review, Ready to certify, Certified, or Exported.

Certification stores record ID/version, certification time and statement, a minimal canonical snapshot, and SHA-256 digest. Certified snapshots are immutable. Editing after certification creates a new draft/version and requires certification again.

### Audit entry

The local audit trail records project changes, inference acceptance/override, exclusion, generation, certification, supersession, and export using action, subject identifier, and timestamp. It must not copy sensitive evidence content.

## 6. Classification and time semantics

Classification is deterministic and local in the alpha. It applies, in order where available:

- prior user-confirmed mappings;
- repository and safe relative-path matches;
- exact/normalized domains;
- project aliases and keywords in sanitized titles/context;
- known local thread association; and
- temporal continuity.

Results use `high`, `medium`, or `low` bands rather than fake numerical precision. High-confidence matches may be prefilled, medium-confidence matches are visibly review-needed, and low-confidence evidence remains unallocated. Every suggestion includes a short explanation.

Optional AI classification is deferred. The business path must not call a provider or require a key. A future AI layer may only use explicit minimized metadata, strip full paths, credential-like strings and URL queries/fragments, return structured uncertainty, and record that remote inference was used.

Intervals are Unix-second half-open ranges `[start, end)`. Overlapping observed intervals are deduplicated before totals are calculated. Imported history visits and editor/save metadata are zero-duration corroborating signals; they do not manufacture foreground time.

## 7. Evidence and review requirements

Each review row shows date/time, duration, app, sanitized context, project and category suggestions, confidence, explanation, and provenance availability. Actions include Accept, Change project, Change category, Split, Exclude, Mark personal/non-work, Mark uncertain, and Bulk accept high-confidence.

Review is local and shares nothing. The interface must keep unreviewed, excluded, unallocated, and low-confidence time visible. A record cannot be certified until every positive-duration item in its scope is reviewed or excluded and every included item has a resolved project and category. Zero-duration contextual signals do not block readiness.

## 8. Record builder

The user selects a date range, template, and projects/components, then generates a draft. The draft reports:

- total tracked time;
- reviewed and unreviewed time;
- excluded and unallocated time;
- allocation by project/category;
- percentages; and
- confidence/review state.

Corrections, splits, exclusions, and assignments recalculate totals immediately. Percentages may not conceal missing time to force a complete-looking result.

## 9. Certification

Certification requires the user to inspect period, template, projects/categories, totals, unresolved state, and exactly which fields will be certified, then affirm:

> I reviewed this record and confirm that, to the best of my knowledge, it reasonably represents my work during this period.

Certification is not a boolean on a mutable draft. Rust canonically serializes the allowlisted snapshot, stores it as a new immutable version, and hashes it. A changed draft cannot reuse the old certification.

## 10. Export and egress

Exports support CSV and JSON. Native saving uses an operating-system save dialog. Browser preview produces clearly synthetic content only. No automatic upload or external submission exists.

Before saving, the preview separates **Will be exported** from **Stays private on this Mac**. Default export fields are limited to certifying name, period, template, project/client, category, hours, percentage, certification statement/timestamp, identifiers/version, and integrity hash.

Raw URLs, browsing rows, full titles, app timelines, local/repository paths, Git evidence, source event IDs, unrelated or personal activity, LLM prompts, and credentials stay private by default. Preview and saved content must use the same serializer and allowlist.

## 11. Privacy, retention, and deletion

Detailed activity stays in local SQLite and expires after 30 days. Optional Chrome-profile import is explicitly selected, limited to 30 days, local, and makes no LLM call. Derived evidence can retain minimal review results after source detail expires and must show lost provenance gracefully.

Certified snapshots remain until deleted because they contain only necessary derived fields, not embedded raw history. Pause/resume, application/domain exclusions, profile authorization, and delete everything remain user-controlled. Delete behavior is logical, not guaranteed forensic erasure.

Provider-backed legacy context features are explicit and secondary. The primary workflow performs no provider egress and no background provider refresh.

## 12. Onboarding and demo data

Onboarding must explain:

1. limited metadata is observed locally;
2. raw evidence remains local;
3. Knov prepares draft records;
4. the user decides what is correct; and
5. nothing is shared until explicit export.

Onboarding works without a provider or Chrome profile and leads into first-project creation. Browser preview is clearly synthetic and includes VS Code, Terminal, GitHub, documentation, collaboration, admin, and excluded personal activity across Search Ranking V2, Authentication, and Internal Operations. It includes high-confidence, ambiguous, unallocated, corrected, and excluded examples plus a prebuilt experimental R&D draft.

## 13. Migration and compatibility

Schema version 6 adds project, evidence, record, certification, and audit storage through ordered SQLite migrations. Existing activity, exclusions, settings, selected browser profiles, corrections, and legacy context data are retained. No migration may reset the database merely to establish the new thesis.

The optional Chrome extension remains a compatibility experiment. Legacy BYOK controls remain accessible under Settings; their provider refresh scheduler is disabled from background operation.

## 14. Acceptance criteria

The P0 workflow is complete when a user can create a project, derive and review evidence, build a record, explicitly certify an immutable version, preview the outbound allowlist, and save valid CSV/JSON without any raw field entering the export.

Verification must cover deterministic and ambiguous classification, overrides/exclusions, interval overlap and boundary handling, zero-duration context signals, aggregation, readiness, immutable versioning and deterministic hashes, export parity/allowlisting, retention separation, pause/exclusion privacy, and existing collection/database regressions.

Alpha success is measured by correctness and trust: users can explain why every included allocation exists, can see unresolved time, and can produce a minimal record without giving the recipient their raw endpoint history.
