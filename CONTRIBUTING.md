# Contributing to Knov

## Setup

Follow the source setup in [README.md](README.md). You need Node.js 22 (see
`.nvmrc`; patched releases `>=22.23.4`, `>=24.22.1` or `>=26.12.1` are
accepted) and the stable Rust toolchain with `rustfmt` and `clippy`.
`rust-toolchain.toml` tracks the latest stable release (it is not pinned to a
version) and installs both components.
The desktop crate uses macOS-only APIs, so Rust checks run on macOS.

```sh
npm ci
```

## Before opening a pull request

Run the full local check and make sure it passes:

```sh
npm run check
```

It runs, in order: desktop and extension typechecks, desktop and extension
tests, `cargo fmt --check`, `cargo clippy -- -D warnings`, and the Rust tests.
CI runs the same checks on every pull request. Use `npm run fmt:rust` to fix
Rust formatting.

- How the test suites are organized and what they cover: [docs/testing.md](docs/testing.md)
- How the app is put together: [docs/architecture.md](docs/architecture.md)

## Commits

Keep commit subjects short and imperative, for example `Add activity icons to
timeline` or `Expand revenue intelligence and discovery`. A conventional
prefix such as `feat:` or `fix:` is welcome (for example
`feat: add contextual work threads and Bedrock support`). Put the reasoning in
the commit body when the change is not obvious.

## Security

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
