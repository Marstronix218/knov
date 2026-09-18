# Testing

## Standard verification

Install dependencies once, then run the desktop checks from the repository root:

```sh
npm install
npm run typecheck --workspace @knov/desktop
npm test --workspace @knov/desktop
npm run build --workspace @knov/desktop
npm run check:rust
npm run test:rust
```

These checks cover the TypeScript contract and interface, synthetic browser-preview behavior, the production Vite build, Rust business rules, migrations, retention, collection helpers, certification, and export boundaries.

When changing the optional Chrome companion, also run:

```sh
npm run typecheck --workspace @knov/chrome-extension
npm test --workspace @knov/chrome-extension
npm run build --workspace @knov/chrome-extension
```

## Business-rule coverage

The endpoint-evidence suite should prove:

- deterministic matching for repositories/paths, domains, aliases, and keywords;
- high/medium/low confidence behavior, ambiguity, and unallocated evidence;
- authoritative manual project/category overrides and exclusions;
- half-open date ranges `[start, end)` and overlap-safe duration aggregation;
- zero-duration Chrome-history/editor signals do not create billable or allocatable time;
- totals and percentages include reviewed, unreviewed, excluded, and unallocated time;
- record versions remain distinct;
- certification requires an explicit accepted statement and a current ready version;
- positive-duration items cannot be certified while unreviewed/unexcluded or unresolved;
- canonical snapshot hashing is deterministic and prior certifications are immutable;
- edits after certification create a new draft/version;
- CSV and JSON parse correctly and contain only approved fields;
- preview content matches saved export content;
- URLs, titles, paths, source events, prompts, and credentials do not enter exports;
- raw retention does not delete certifications or smuggle raw detail into them; and
- excluded apps/domains do not contribute to derived evidence.

Existing collection, database, pause, deletion, sanitization, and extension tests remain regression coverage for the reused capture layer.

## Synthetic browser demo

```sh
npm run dev
```

The browser preview is an explicitly synthetic workspace. It is useful for testing navigation, review actions, record totals, certification state transitions, export preview, and responsive layout. It cannot validate native collection, SQLite persistence, Keychain, operating-system permissions, Chrome import, save dialogs, or filesystem output.

Manual browser flow:

1. Confirm primary navigation is **Evidence**, **Projects**, **Records**, **Review & Certify**, **Exports**, **Activity**, and **Settings**.
2. Confirm the workspace is marked synthetic and contains Search Ranking V2, Authentication, and Internal Operations.
3. Inspect high-confidence, ambiguous, unallocated, corrected, and excluded evidence.
4. Attempt certification with unresolved positive-duration evidence and confirm it is blocked.
5. Resolve or exclude every blocking item, certify, and record the version/hash shown.
6. Revise the record and confirm the previous certification remains unchanged while the new version requires certification.
7. Compare **Will be exported** and **Stays private on this Mac** with the CSV and JSON previews.

## Native alpha checklist

Run the native app with:

```sh
npm run dev:desktop
```

1. Complete onboarding without a provider key or Chrome profile.
2. Confirm collection starts under explicit user control.
3. Create and edit a project with domain, keyword, repository, and path signals.
4. Deny Accessibility and verify degraded app-only collection; grant it and verify title evidence after restarting if macOS requires it.
5. Optionally select one Chrome profile; confirm import is limited to 30 days and does not make a provider call.
6. Generate evidence and inspect source, time, sanitized context, confidence, and explanation.
7. Verify pause and excluded app/domain behavior before building a record.
8. Review all positive-duration evidence, generate a record, and confirm totals do not double-count overlaps.
9. Certify explicitly; change the underlying draft and confirm the stored certification does not change.
10. Export CSV and JSON with the native save dialog. Inspect both files for the allowlisted fields and absence of raw metadata.
11. Let or simulate raw rows expiring and confirm certification remains while detailed provenance reports unavailable.
12. Invoke **Delete everything** and confirm app-owned rows, certifications, settings, and configured Keychain credentials are removed or failures are reported.

The optional legacy BYOK controls can be tested separately from the business workflow. A provider must never be required for onboarding, deterministic classification, certification, or export.

## Inspect local state

With the app stopped, the macOS database is normally:

```sh
KNOV_DB="$HOME/Library/Application Support/com.knov.desktop/knov.sqlite3"
sqlite3 "$KNOV_DB" 'PRAGMA user_version;'
sqlite3 "$KNOV_DB" '.tables'
sqlite3 "$KNOV_DB" \
  'SELECT source, COUNT(*) FROM activity_events GROUP BY source;'
```

Stop the app before inspection so an uncheckpointed WAL is not mistaken for missing data. Do not edit the database directly; migrations and invariants belong to the Rust core.

## Known alpha coverage gaps

- no automated real-macOS Accessibility or save-dialog test;
- no real Chrome-profile import or Native Messaging end-to-end automation;
- no packaged-app, signing, notarization, update, or installer test;
- no multi-user identity proof, trusted timestamp authority, or hosted hash verifier;
- no forensic-erasure test; and
- no independent security assessment.
