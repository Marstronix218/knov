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

### Changed

- README, tester guide, PRD, and docs reconciled with the app's current behavior and supported platform.

## [0.2.0] - 2026-10-20

### Added

- Revenue intelligence: evidence-backed scope and commitment findings, interview clarification, internal drafts, and tracked outcomes.
- Simulation sandbox with fictional businesses and scripted interviews to validate revenue findings.
- Four-step setup wizard on first native launch.
- Static web landing page published to GitHub Pages.
- Release workflow that builds a draft macOS DMG from a version tag.

[Unreleased]: https://github.com/Marstronix218/knov/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Marstronix218/knov/releases/tag/v0.2.0
