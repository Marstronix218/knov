# Security Policy

## Supported versions

Only the latest release of Knov receives security fixes. Please reproduce an
issue on the newest release (or `main`) before reporting it.

| Version        | Supported |
| -------------- | --------- |
| Latest release | Yes       |
| Older releases | No        |

## Reporting a vulnerability

Please report vulnerabilities privately. Do not open a public issue, pull
request, or discussion for a suspected security problem.

- Preferred: open a private report through
  [GitHub Security Advisories](https://github.com/Marstronix218/knov/security/advisories/new)
  on `Marstronix218/knov`.
- Alternatively, contact the maintainer listed in the `author` field of the
  root `package.json` (Noriaki Kishida) directly.

Include the affected version, macOS version, reproduction steps, and the
impact you observed. You will receive an acknowledgement, and a fix or
mitigation plan will be shared before any public disclosure.

## Scope

Knov is a local-first app that collects activity metadata on your Mac. What
the app is designed to protect, and the boundaries it assumes, are described
in:

- [docs/threat-model.md](docs/threat-model.md)
- [docs/privacy-model.md](docs/privacy-model.md)

Reports that show Knov collecting, storing, or sending data beyond what those
documents describe are in scope, as are issues in the desktop app, the Rust
backend, the optional Chrome companion extension, and the release workflows.
