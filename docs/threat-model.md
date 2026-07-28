# Threat model

## Security objective

The alpha aims to prevent accidental collection and unnecessary network egress,
keep provider credentials out of the activity database and frontend, and accept
browser events only from a locally paired Chrome extension.

It is not designed to protect data from malware, a compromised macOS account,
an administrator/root user, forensic disk analysis, or a compromised AI
provider.

## Assets

- detailed browsing and foreground-application history
- window and page titles, URLs, and extracted search queries
- generated profile, recommendations, and user corrections
- OpenAI or Anthropic API key
- extension pairing token and unfinished active-tab session

## Trust boundaries

| Boundary | Existing control | Residual risk |
| --- | --- | --- |
| React to Rust IPC | Tauri command allowlist, strict CSP, and typed arguments | A compromised bundled frontend could invoke exposed commands |
| Rust to SQLite | Single in-process writer, parameterized queries, local path | Database is not application-level encrypted |
| Rust to Keychain | Apple-native Keychain backend; keys never returned by commands | A compromised user session or permissive Keychain ACL may access keys |
| Extension to native host | Chrome `allowed_origins`, protocol version, token, extension-ID binding | Registration is manual; pairing token is stored in plaintext local stores |
| Native host to Rust core | Mode-0600 per-user Unix socket and bounded message size | Same-user compromise remains in scope |
| Rust core to provider | HTTPS with direct provider authentication | Provider receives and may retain the disclosed digest/chat data |

## Controls implemented

- The extension has no content scripts and incognito use is disabled.
- Only active HTTP(S) tab metadata is observed; completed events are not queued on disk.
- Native messages are limited to 256 KiB and validated before ingestion.
- The first extension ID that authenticates with a pairing token is bound in
  SQLite; later different IDs are rejected.
- Loopback HTTP accepts only a bearer token and is labeled development-only.
- Event fingerprints deduplicate repeated ingestion.
- Profile requests use aggregate rows and domain-only website values rather
  than sending the raw database.
- Provider status errors are converted to safe user-facing messages without
  echoing credentials or response bodies.
- User corrections are stored separately and supplied as authoritative context.

## Material residual risks

### Sensitive metadata

Window titles and URLs can reveal document names, account identifiers, health or
financial topics, and search intent. Current redaction is narrow and does not
identify general secrets or sensitive categories. Exclusions require exact app
names and normalized domain matching.

### Local data exposure

SQLite uses WAL mode without SQLCipher or field encryption. macOS account
security, FileVault, filesystem permissions, backups, and endpoint hygiene are
the primary protections. The pairing token is also available to processes that
can read the user's local files or Chrome profile.

### Development transport

The localhost fallback has no TLS and expands the local attack surface. The
bearer token prevents unauthenticated ingestion, but the mode should be used
only for local development. Native Messaging remains the intended transport.

### Extension permissions

The extension requests `tabs`, `storage`, `alarms`, and `nativeMessaging`, plus
loopback host permissions for development. The `tabs` permission can expose tab
metadata to extension code even though Knoveyla intentionally queries only the
active tab.

### Provider egress

The provider sees all data described in [Privacy model](privacy-model.md).
Prompts reduce harmful inference but cannot guarantee a provider will follow
them or return valid structured data. Provider compromise, account retention,
abuse monitoring, and legal disclosure are outside the local application's
control.

### Deletion limits

Row deletion and Keychain deletion are logical application operations, not
verified secure erasure. The in-app action removes its host manifest but does
not clear Chrome extension storage, delete the SQLite file, sanitize WAL/free
pages, remove backups, or delete provider-held data.

### Alpha hardening gaps

- Native helper registration still requires the extension ID in a source build.
- Desktop and extension domain-exclusion lists are configured separately.
- There is no code signing, notarization, update channel, or release-integrity
  process documented for this source alpha.
- Security behavior has unit/integration coverage but has not undergone an
  independent penetration test.

## Recommended operating posture

- Use a non-production provider key with a strict spending limit.
- Enable FileVault and a strong macOS login password.
- Exclude sensitive applications and domains in both the app and extension.
- Prefer Native Messaging over loopback HTTP.
- Keep Chrome extension developer mode limited to this inspected build.
- Do not use the alpha on shared or managed computers without understanding
  local administrator access and backup policy.
- Remove the extension and its stored data after testing if its pairing
  configuration must not remain in Chrome.
