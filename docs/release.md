# Shipping a tester build

How to publish a downloadable Knov build for early-access testers.

## One-time setup

1. **Make the repository public** (or host the DMG somewhere public). Testers
   download from GitHub Releases, which requires access to the repository.
2. **Set where feedback goes.** In GitHub: *Settings → Secrets and variables →
   Actions → Variables*, add one of:
   - `FEEDBACK_EMAIL`: the in-app **Send feedback** button opens a pre-filled
     draft to this address in the tester's mail app.
   - `FEEDBACK_URL`: a form (Tally, Google Forms, Typeform). The app copies the
     tester's answers and opens the form. This is preferred when set, because
     many people use webmail rather than Mail.app.

   With neither set, feedback opens a pre-filled GitHub issue, which requires a
   GitHub account.
3. **Optional but strongly recommended: sign and notarize.** Without an Apple
   Developer ID ($99/year), every tester has to click **Open Anyway** in
   System Settings on first launch, which loses some testers. Add these
   repository secrets to sign and notarize automatically:
   `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_SIGNING_IDENTITY` (e.g. `Developer ID Application: Name (TEAMID)`),
   `APPLE_ID`, `APPLE_PASSWORD` (app-specific password), and `APPLE_TEAM_ID`.

## Each release

1. Bump the version in `apps/desktop/src-tauri/tauri.conf.json`,
   `apps/desktop/src-tauri/Cargo.toml`, and both `package.json` files.
2. Commit, then tag and push:

   ```sh
   git tag v0.2.0
   git push origin main v0.2.0
   ```

3. The **Release macOS build** workflow runs the tests and builds a universal
   (Apple Silicon and Intel) DMG. It creates a **draft** release.
4. Open the draft under *Releases*, check the notes, and click **Publish**.
   Share `https://github.com/Marstronix218/knov/releases/latest` together with
   `TESTERS.md`.

To build locally instead, run `npm run build:desktop`. The DMG is written to
`apps/desktop/src-tauri/target/release/bundle/dmg/`. Set `VITE_FEEDBACK_EMAIL`
or `VITE_FEEDBACK_URL` in your shell before building to configure feedback.

## Notes

- macOS ties Accessibility and Automation permissions to the app's code
  signature. Ad-hoc builds get a new signature each release, so testers may
  need to re-enable Accessibility after updating. A Developer ID signature
  avoids this.
- The **Check for updates** button in Settings opens the latest release page.
  The app itself never contacts GitHub.
