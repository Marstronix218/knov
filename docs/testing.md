# Testing

## Full verification

Install dependencies once:

```sh
npm install
```

Run the repository checks from the root:

```sh
npm run typecheck
npm test
npm run build
npm run check:rust
npm run test:rust
```

These commands cover:

- desktop TypeScript and React type checking
- desktop component and browser-preview API tests
- extension configuration and activity-session tests
- production Vite builds for both npm workspaces
- Rust compilation, database migration/retention tests, digest handling, and
  collector helper tests

## Targeted checks

Desktop frontend:

```sh
npm run typecheck --workspace @knoveyla/desktop
npm test --workspace @knoveyla/desktop
npm run build --workspace @knoveyla/desktop
```

Chrome extension:

```sh
npm run typecheck --workspace @knoveyla/chrome-extension
npm test --workspace @knoveyla/chrome-extension
npm run build --workspace @knoveyla/chrome-extension
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

## Manual alpha checklist

Automated tests do not cover macOS permission dialogs, Keychain UI, a real
Chrome profile, or live provider accounts. Before an alpha handoff:

1. Launch on an Apple Silicon Mac running macOS 26.
2. Confirm collection begins disabled before consent.
3. Complete onboarding with one selected Chrome profile and a limited-use
   provider key.
4. Deny Accessibility and verify degraded status; grant it and verify window
   titles appear after restarting if necessary.
5. Import history and confirm the first profile succeeds.
6. Register and pair the extension using [Alpha setup](alpha-setup.md).
7. Focus two ordinary HTTP(S) tabs and verify duration events reach Activity.
8. Verify incognito, `chrome://` pages, excluded domains, and subdomains are not
   collected.
9. Stop the app, create an extension event, restart the app, and verify the
   failed event is not replayed.
10. Pause the app and verify the extension follows the native state and no new app-owned activity rows
    are added.
11. Exercise OpenAI or Anthropic validation, profile refresh, and chat with a
    non-production key.
12. Add a profile correction, refresh, and confirm the correction remains.
13. Dismiss a recommendation and confirm it leaves the dashboard.
14. Invoke **Delete everything**, then verify app-owned rows are gone, default
    settings return, old pairing fails, and provider keys are unavailable.
15. Clear/remove the extension separately and remove the Native Messaging
    manifest when the test is complete.

## Inspect local state

With the app stopped, the macOS database is normally:

```sh
KNOVEYLA_DB="$HOME/Library/Application Support/com.knoveyla.desktop/knoveyla.sqlite3"
sqlite3 "$KNOVEYLA_DB" '.tables'
sqlite3 "$KNOVEYLA_DB" \
  'SELECT source, COUNT(*) FROM activity_events GROUP BY source;'
```

Stop the app before direct inspection to avoid mistaking an uncheckpointed WAL
state for missing data. Do not edit the database; migrations and invariants are
owned by the Rust core.

## Known coverage gaps

- no automated real-macOS Accessibility test
- no real Chrome Native Messaging end-to-end test
- no provider contract test against live OpenAI or Anthropic APIs
- no packaged-app, code-signing, notarization, update, or installer test
- no secure-deletion claim or forensic-erasure test
- no independent security assessment
