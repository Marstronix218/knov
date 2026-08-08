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
- desktop component and browser-preview API tests
- the production desktop Vite build
- Rust compilation, database migration/retention tests, digest handling, and
  collector helper tests

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
12. Invoke **Delete everything**, then verify app-owned rows are gone, default
    settings return, and provider keys are unavailable.

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
```

Stop the app before direct inspection to avoid mistaking an uncheckpointed WAL
state for missing data. Do not edit the database; migrations and invariants are
owned by the Rust core.

## Known coverage gaps

- no automated real-macOS Accessibility test
- no real Chrome Native Messaging end-to-end test
- no provider contract test against live OpenAI, Anthropic, or Amazon Bedrock APIs
- no packaged-app, code-signing, notarization, update, or installer test
- no secure-deletion claim or forensic-erasure test
- no independent security assessment
