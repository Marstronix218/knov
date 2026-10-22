# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- CI workflow running typechecks, tests, the extension build, `cargo fmt`, `cargo clippy`, and Rust tests.
- Dependabot updates for npm, Cargo, and GitHub Actions.
- `npm run check` and Rust lint/format scripts, plus `SECURITY.md`, `CONTRIBUTING.md`, and `LICENSE`.
- Settings → Export my data: writes local data as one JSON file to Downloads, with API keys, connector tokens, and the pairing token excluded.
- Error boundary that keeps a failing view recoverable, plus app-wide error notices for failures with no inline place to show.
- Review-before-import for uploaded documents: extracted text is shown for editing and nothing is stored until you import it.
- Chrome companion card in Settings (Labs) to register the native host and copy the pairing token.
- Settings for the prediction display threshold, the time between prediction sets, and the collection sampling interval.
- ⌘K "Ask with context" command that opens Ask with the selected thread attached.
- Keyboard shortcuts dialog, opened with `?`.
- Discard-changes guard on dialogs with unsaved edits.
- Ask shows the attached thread as a removable context chip and keeps the conversation for the session.
- Sources: guided Gmail and Slack connection cards. Gmail signs in with your own Google Desktop client, keeps a refresh token in Keychain so sign-in survives token expiry, and syncs incrementally (only new mail inside the approved search and dates).
- Sources: in-app Slack sign-in with your own Slack app. Slack redirects to a static code page on the project site (`apps/web/slack/callback.html`); you paste the one-time code into Knov, which exchanges it on your Mac for a read-only user token. Channels are picked by name (up to 10) instead of typed IDs.

### Changed

- README, tester guide, PRD, and docs reconciled with the app's current behavior and supported platform.
- Disconnecting Gmail revokes the stored refresh token. Pasting a token remains available under Advanced. Live Google and Slack sign-in, refresh, and channel listing have not yet been exercised against the real services.

## [0.2.0] - 2026-10-20

### Added

- Revenue intelligence: evidence-backed scope and commitment findings, interview clarification, internal drafts, and tracked outcomes.
- Simulation sandbox with fictional businesses and scripted interviews to validate revenue findings.
- Four-step setup wizard on first native launch.
- Static web landing page published to GitHub Pages.
- Release workflow that builds a draft macOS DMG from a version tag.

[Unreleased]: https://github.com/Marstronix218/knov/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Marstronix218/knov/releases/tag/v0.2.0
