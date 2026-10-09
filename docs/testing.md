# Testing

## Baseline MVP verification

Install dependencies once:

```sh
npm install
```

Run the desktop MVP checks from the root:

```sh
npm run typecheck --workspace @knov/desktop
npm test --workspace @knov/desktop
npm run build --workspace @knov/desktop
npm run check:rust
npm run test:rust
```

These commands cover:

- desktop TypeScript and React type checking
- desktop component and browser-preview API tests, including the Ready for you
  inbox, run review and approval, Workflows and Skills, Delegated work,
  the command menu, settings error handling, and setup without a provider
- the production desktop Vite build
- Rust compilation, database migration/retention tests (including databases
  with a divergent `user_version`), prediction state, persistence/evaluation,
  workflow candidates and calibration, digest handling, and collector helper
  tests
- work-agent tests: event normalization, workflow mining and quality filters,
  opportunity scoring, goal inference, skill generation and validated edits,
  permission precedence, unattended rules, budgets, the kill switch,
  plan → approve → execute → verify → journal, stop-on-exception, rollback,
  interrupted-action recovery, proposals, and Delete everything

Agent execution tests use an in-memory action host; the automated suite never
opens windows or runs project commands.

Prediction tests use mocked provider responses. The automated suite must not
require a provider credential or make a live provider request.

## Targeted checks

Desktop frontend:

```sh
npm run typecheck --workspace @knov/desktop
npm test --workspace @knov/desktop
npm run build --workspace @knov/desktop
```

### Optional extension compatibility lane

The extension is an implemented post-MVP experiment, not a baseline onboarding
or release gate. Run these checks when changing or evaluating that companion:

```sh
npm run typecheck --workspace @knov/chrome-extension
npm test --workspace @knov/chrome-extension
npm run build --workspace @knov/chrome-extension
```

Rust core:

```sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets
```

## Browser preview versus native app

```sh
npm run dev
```

This starts only the Vite browser preview. It intentionally uses sample data
when the Tauri runtime is absent. It can validate layout and interactions, but
it cannot prove collection, SQLite, Keychain, Chrome import, Native Messaging,
provider calls, autostart, or deletion behavior.

Use the native development app for integration checks:

```sh
npm run dev:desktop
```

The optional extension bridge is disabled in that baseline command. Use
`npm run dev:with-extension` only for the extension compatibility checklist.

## Manual baseline alpha checklist

Automated tests do not cover macOS permission dialogs, Keychain UI, a real
Chrome profile, or live provider accounts. Before an alpha handoff:

1. Launch on an Apple Silicon Mac running macOS 26.
2. Confirm collection begins disabled before consent.
3. Complete onboarding with one selected Chrome profile and a limited-use
   provider key.
4. Deny Accessibility and verify degraded status; grant it and verify window
   titles appear after restarting if necessary.
5. Import history and confirm the first profile succeeds.
6. Save files in a supported editor and verify only safe workspace-relative
   paths appear; hidden, generated, dependency, and credential paths must not.
7. Pause desktop collection and verify no new app-owned activity rows are added.
8. Exercise OpenAI, Anthropic, or Amazon Bedrock validation, profile refresh,
   and chat with a non-production key.
9. Verify selected-thread context is sanitized, token-budgeted, and its
    context-economics record is stored only in local SQLite.
10. Add a profile correction, refresh, and confirm the correction remains.
11. Dismiss a recommendation and confirm it leaves the dashboard.
12. Confirm the Prediction Experiment is disabled by default and produces no
    prediction request while disabled or collection is paused.
13. Enable the experiment, create enough meaningful activity, and confirm an
    automatically generated provider batch and deterministic baseline are
    stored no more often than the configured cooldown permits.
14. Confirm only unexpired candidates at or above the display threshold appear
    under **Likely next**; submit correct, not-what-I’m-doing, and dismiss
    feedback and verify it appears in **Local prediction history**.
15. Resume a safe predicted thread/resource and verify that Knov does not run a
    command, edit a file, send a message, or submit a form.
16. After the prediction horizon, verify a local outcome, status, and match
    score are recorded and the provider/baseline aggregate metrics update.
17. Force a prediction-only provider or parsing failure and verify collection,
    the existing Now fallback, profile refresh, and chat remain independently
    usable. Provider-dependent actions may still report their own connection
    error when the provider itself is unavailable.
18. With a few days of repeated activity, open **Workflows**, choose
    **Rescan**, and confirm workflows show evidence with app names, domains,
    and paths only. Dismiss one and confirm it stays dismissed after another
    rescan.
19. Create a skill, approve a project folder under **Agent → Permissions**, and
    point the skill's terminal step at it. Choose **Run now**, untick one
    action, and approve the rest; confirm only approved actions run, checks
    report output and exit code, and the declined step is journaled.
20. Undo the resume-brief draft; then run again, edit the draft in another
    app, and confirm **Undo** refuses to delete your edited version.
21. Approve the same action five times and confirm a permission suggestion
    appears; accept it and confirm the next scheduled or context-triggered run
    executes only non-window actions automatically and leaves page/app opens
    in **Ready for you**.
22. Pause the agent, approve a pending run, and confirm nothing executes and
    the actions are skipped. Quit Knov during a long check and confirm the
    action is marked failed on the next launch.
23. Invoke **Delete everything**, then verify app-owned rows (including agent
    workflows, skills, runs, permissions, folders, goal reviews, and
    snapshots) and the agent `drafts` folder are gone, default settings
    return, and provider keys are unavailable.

## Optional extension manual checklist

This compatibility lane does not block the MVP handoff:

1. Register and pair the extension using [Alpha setup](alpha-setup.md#optional-chrome-extension-setup).
2. Focus two ordinary HTTP(S) tabs and verify duration events reach Activity.
3. Verify incognito, `chrome://` pages, excluded domains, and subdomains are not
   collected.
4. Stop the app, create an extension event, restart the app, and verify the
   failed event is not replayed.
5. Pause the app and verify the extension follows the native state and no new
   app-owned activity rows are added.
6. Invoke **Delete everything** and verify the old extension pairing fails.
7. Clear/remove the extension separately and remove the Native Messaging
   manifest when the test is complete.

## Inspect local state

With the app stopped, the macOS database is normally:

```sh
KNOV_DB="$HOME/Library/Application Support/com.knov.desktop/knov.sqlite3"
sqlite3 "$KNOV_DB" '.tables'
sqlite3 "$KNOV_DB" \
  'SELECT source, COUNT(*) FROM activity_events GROUP BY source;'
sqlite3 "$KNOV_DB" \
  'SELECT event_type, COUNT(*) FROM product_events GROUP BY event_type;'
sqlite3 "$KNOV_DB" \
  'SELECT prediction_source, evaluation_status, COUNT(*) FROM predictions GROUP BY prediction_source, evaluation_status;'
sqlite3 "$KNOV_DB" \
  'SELECT status, COUNT(*) FROM workflows GROUP BY status;'
sqlite3 "$KNOV_DB" \
  'SELECT action_type, decision, status, COUNT(*) FROM agent_actions GROUP BY 1, 2, 3;'
```

Stop the app before direct inspection to avoid mistaking an uncheckpointed WAL
state for missing data. Do not edit the database; migrations and invariants are
owned by the Rust core.

## Known coverage gaps

Workflow Discovery adds synthetic structured-response and native persistence
tests, graph provenance/revision/deletion tests, frontend interview/editor tests,
and deferred-response tests protecting historical graph inspection. A disk-backed
test closes and reopens SQLite to verify saved interviews and graph revisions
survive restart. See [Workflow intelligence](workflow-discovery.md) for scope and
the remaining connector-to-Skill end-to-end milestone.

- no automated real-macOS Accessibility test
- no real Chrome Native Messaging end-to-end test
- no provider contract test against live OpenAI, Anthropic, or Amazon Bedrock APIs
- no long-running real-activity validation of prediction accuracy or personal
  historical-pattern quality; workflow-mining thresholds were checked once
  against a copy of one real 30-day database
- no automated test that runs real project test commands or opens real windows
- no packaged-app, code-signing, notarization, update, or installer test
- no secure-deletion claim or forensic-erasure test
- no independent security assessment
