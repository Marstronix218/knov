# Revenue intelligence

Revenue extends Knov's local context layer. It does not change activity collection
or replace the general workflow interviewer. Imported contracts, deliberately
selected communications, explicit work evidence, and user corrections support
commercial review. All customer-facing output is an internal draft.

## Run the desktop demo

```sh
npm install
npm run dev:desktop
```

Complete the normal first-run setup, open **Revenue**, choose **Open isolated demo**, then
**Load reproducible demo**. No AI-provider key or Gmail/Slack account is required for
the deterministic commercial review loop. Fixtures are isolated from real data
and use a fixed reference clock, so an overdue proposal remains reproducible.

The Acme Analytics agreement covers two dashboards. The third-dashboard request
and explicitly linked work support a possible scope change; absent approval is
an uncertainty. Answer the clarification, prepare a change-order draft, edit it,
and approve it for manual use. Approval does not send the draft. Record the later
commercial action and payment separately. The seed also includes an overdue
proposal, a milestone whose billing needs checking, and already-approved work
that should not produce a scope opportunity.

The browser Vite preview is not a desktop database or a live connector. Use the
Tauri application to exercise persistence, imports, and source authorization.

## Evidence and financial boundaries

- Agreement extraction is a conservative local proposal for user review, not a
  legal interpretation. Unknown scope, dates, and amounts remain unknown.
- Scope candidates require an explicit request and associated work. Overdue
  commitments require a reliable due date. Missing invoice coverage produces
  billing verification, not a claim of lost revenue.
- Source text remains data. The deterministic detector does not execute source
  instructions or send imported content to an AI provider.
- User clarification is authoritative context for this project. It is distinct
  from independent verification of a financial result.
- Money uses integer cents with currency (USD, EUR, GBP, CAD, AUD). Potential value is separate from
  recovery with user-checked receipts; invoice issuance alone is never payment.
  Receipt checking is user-attested and retains `user_confirmed` provenance.
  There is no independently verified payment integration in this slice.
- A source reference is not proof that an external action occurred. Record
  supporting evidence and distinguish self-reports from user-attested receipt checks.

## Connections and document imports

See [connector configuration](revenue-connectors.md) for the actual authorization
flow, scopes, credentials, retrieval bounds, and disconnect behavior. Live calls
require your own provider configuration; fixture data is never shown as a live
sync. Plain text and Markdown can be imported locally. Agreement persistence is limited to 64 KB of UTF-8 text. PDF extraction requires
`pdftotext` from Poppler; scanned PDFs requiring OCR are unsupported.

## Verification

Run the supported repository checks:

```sh
npm run typecheck --workspace @knov/desktop
npm test --workspace @knov/desktop
npm run build --workspace @knov/desktop
npm run check:rust
npm run test:rust
```

Revenue tests cover commercial detection, corrections, evidence provenance,
persistence, demo isolation, permission boundaries, and financial attribution.
The existing discovery and threading tests remain part of the regression suite.
Computer-use smoke testing could not run: the available UI automation tool failed
to start its app-server (`No such file or directory`). Component tests and
production builds cover the UI; no native visual screenshot is claimed.

## Scope and next step

This is an alpha commercial review loop, not a CRM or accounting system. There
are no email-send, Slack-post, contract-change, invoice-issue, or payment APIs.
Commercial Skill suggestions prepare a procedure for review at the existing
workflow/Skill boundary; they do not add a second runtime. Provider credentials, live sync, and PDF extraction require
external configuration and are not proven by fixture tests.

The smallest next customer-validation improvement is reconciliation with a
single explicitly authorized invoice source, including complete coverage dates.
That would distinguish genuinely unbilled milestones from missing billing data.

## Retention and deletion

Automatically imported Gmail and Slack excerpts use the existing 30-day
retention window; unreviewed derived drafts are redacted with their sources.
Deliberately imported agreements and approved drafts remain saved until user
deletion. Deleting a source also clears copied content from its dependent drafts
and commitments while preserving non-content decisions and financial audit
records. The source settings control can delete imported content; disconnect
first if future explicit sync must not import it again. Full-data deletion
includes revenue records, interview records, and both source Keychain entries.
Demo clarification sessions are excluded from normal interview/workflow
retrieval so fictional truths do not enter actual memory.

Payment metrics count one latest user-attested receipt per opportunity, with
identical receipt references counted once across links. Split payments, refunds,
and accounting reconciliation are not implemented. Detection is a conservative
heuristic for reviewed evidence, not general contract understanding: it supports
the dashboard scope example and recognizable subject matches; ambiguous or
complex terms require manual review. Synced messages first need a user-reviewed
classification, preserving their original source references. No revenue source
content is sent to a model by this subsystem.
