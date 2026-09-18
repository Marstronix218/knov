# Design

## Source of truth

- Status: Active technical-alpha direction
- Product: privacy-preserving endpoint evidence and user-certified business records
- Primary surface: native macOS desktop app
- Secondary surfaces: synthetic browser preview, optional Chrome companion, legacy personal context in Settings
- Core contract: evidence → inference → human review → immutable certification → constrained export

The R&D Allocation template is an experimental demonstration. Interface copy must not imply tax, legal, accounting, employment, audit, or regulatory compliance.

## Product thesis

Knov turns local work activity into reviewable, purpose-specific business records without exposing the underlying raw activity. Its value comes from the boundary between evidence and disclosure: the user can inspect enough detail to make a decision, while a recipient gets only the record the user approved.

The product should feel private, inspectable, precise, calm, and controlled. It should never resemble an employer surveillance dashboard, productivity scoreboard, or forensic identity product.

## Information architecture

Primary navigation, in order:

1. **Evidence** — current evidence health, queue, and provenance
2. **Projects** — work units and local matching signals
3. **Records** — templates, periods, drafts, totals, and versions
4. **Review & Certify** — resolve evidence and attest to a ready record
5. **Exports** — inspect and save certified artifacts
6. **Activity** — raw local timeline and source inspection
7. **Settings** — collection, exclusions, retention, deletion, source authorization, and legacy controls

Now, Threads, Memory, personal chat, recommendations, context economics, and provider configuration are secondary legacy context in Settings. They must not compete with the main thesis or trigger background provider activity.

## Core workflows

### Onboarding

Onboarding uses plain statements rather than requiring configuration:

1. Knov observes limited activity metadata locally.
2. Raw evidence stays on this Mac.
3. Knov prepares draft records from local rules.
4. The user reviews every included allocation.
5. Nothing is shared until explicit export.

The user may continue without an AI provider, API key, Chrome profile, extension, or Accessibility permission. The next useful action is creating a first project. Chrome selection is explicitly optional, imports at most 30 days locally, and makes no LLM call.

### Evidence

Lead with the amount ready for review and a single **Review evidence** action. Summary metrics distinguish tracked, reviewed/allocated, needs review, excluded, and unallocated time.

Each evidence row shows:

- observed date/time, duration, application, and source;
- safe display context;
- suggested project and category;
- high, medium, or low confidence;
- a short “Why this suggestion” explanation;
- review state and whether original source detail remains available.

Visual labels must distinguish observed, inferred, and user-corrected values. Never render an inference as confirmed intent. Low confidence remains unallocated.

### Projects

Projects are compact work-unit cards or rows with name, client/external identifier where present, active/archived state, color, and a concise summary of aliases, keywords, domains, repositories, and paths. Editing rules should feel like teaching a local classifier, not configuring enterprise automation.

### Records

The builder asks for period, template, and projects/components, then displays a matrix of project/category allocations. Keep tracked, reviewed, unreviewed, excluded, and unallocated totals visible near the matrix. Recalculation follows every correction, split, exclusion, or assignment.

Template labels:

- Generic Project Allocation
- R&D Allocation — Demo / Experimental
- Professional Services Allocation

Experimental/legal boundaries belong beside the R&D template and certification action, not hidden in a footer.

### Review & Certify

Review provides Accept, Change project, Change category, Split, Exclude, Personal/non-work, Uncertain, and Bulk accept high-confidence actions. State clearly: “Review happens locally. Nothing is shared.”

A readiness panel shows the covered period, template, projects/categories, totals, and blockers. Certification stays disabled until every positive-duration item is reviewed or excluded and included items have resolved project and category. Zero-duration history/editor context should be visible but not block readiness.

The certification statement is:

> I reviewed this record and confirm that, to the best of my knowledge, it reasonably represents my work during this period.

After certification, show record version, time, and integrity hash. Editing creates a new draft/version; the prior certification remains visibly fixed.

### Exports

The export preview is the most explicit privacy boundary in the interface. Use two visually distinct panels:

**Will be exported**

- certified identity entered by the user;
- period and template;
- project/client and category allocations;
- hours and percentages;
- certification time, version, IDs, statement, and hash.

**Stays private on this Mac**

- browsing history and raw URLs;
- window/page titles and application timeline;
- local and repository paths;
- source events and provenance detail;
- excluded/personal activity;
- prompts and credentials.

CSV and JSON controls preview exact content before saving. Native uses a save dialog and must never imply upload or submission. Browser preview is visibly synthetic.

## Content principles

1. **Observed before inferred.** Start with facts, then name the suggestion and why it exists.
2. **Human decisions are authoritative.** Use clear “You changed” or “User reviewed” labels.
3. **Uncertainty remains visible.** Do not hide unallocated time or invent precise confidence percentages.
4. **Review is local.** Repeat this at the decision point, where it reduces anxiety.
5. **Certification is specific.** Show the exact version and fields being attested to.
6. **Export is a boundary.** Use “save/export” language, never automatic “sync/send/submit.”
7. **No compliance theater.** Avoid “IRS compliant,” “audit proof,” “verified employee,” or similar claims.

## Visual language

Retain Knov's near-black foundation and chartreuse accent, using the accent for primary actions and trusted completion states rather than decoration. Warm off-white reading surfaces support review; restrained blue identifies observed evidence; amber indicates unresolved or uncertain state; red is reserved for destructive or blocking conditions.

Use the native system type family, readable 13–14px minimum body copy, clear tabular numerals for hours, an 8px spacing base, 12–16px surface radii, and shallow elevation. Reduce dense bordered panels. The evidence list and allocation matrix should scan like a careful ledger rather than an analytics dashboard.

Status should never depend on color alone. Pair color with text and iconography. Controls need keyboard focus, labels, and sufficiently large targets. Long titles and paths are truncated visually with an accessible full local value only where privacy-safe.

## Required states

Every primary screen supports loading, empty, populated, error, and unavailable-source states. Important examples:

- no collection yet → explain sources and offer Resume;
- no project → create first project;
- evidence source expired → retain review result and show provenance unavailable;
- record blocked → list exact unresolved positive-duration items;
- no certification → explain that only certified versions can export;
- browser preview → persistent synthetic-data marker;
- native save failure → preserve preview and allow retry.

## Synthetic demo system

The browser preview depicts a software engineer switching among VS Code, Terminal, GitHub, documentation, collaboration, and internal/admin work. Projects are Search Ranking V2, Authentication, and Internal Operations. The evidence set includes high-confidence, ambiguous, unallocated, user-corrected, and excluded personal examples, plus a prebuilt experimental R&D draft.

The demo should support this narrative without external services: inspect inference, resolve blockers, certify, compare private/exported fields, preview CSV/JSON, revise, and observe that a new version requires a new certification.

## Responsive behavior

The alpha is desktop-first. At narrower widths, the sidebar collapses, metric cards wrap, record matrices gain horizontal scrolling with pinned row labels, and evidence actions move below context. Never hide review status, unresolved time, or export privacy panels solely to fit the viewport.

## Non-goals

- employer, manager, or accountant login;
- productivity scores or employee ranking;
- automatic network export;
- screenshots, OCR, audio, keystrokes, page bodies, clipboard, or form capture;
- a dynamic compliance rules engine;
- AI classification in the alpha business path; and
- replacing the retained source-specific Activity and Settings controls.
