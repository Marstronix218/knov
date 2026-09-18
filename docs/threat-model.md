# Threat model

## Security objective

Knov aims to prevent accidental collection, silent egress, and disclosure of raw endpoint activity through a business record. A third party receives only an explicitly certified, previewed, purpose-specific export. The alpha is not designed to withstand malware, a compromised macOS account, an administrator/root user, forensic disk analysis, coercion outside the product, or a compromised optional AI provider.

## Assets

- raw application, browser, title, URL, query, and editor/Git metadata;
- project rules, derived evidence, review choices, and exclusions;
- draft and certified business records;
- certification statements, version identifiers, and integrity hashes;
- local audit entries;
- optional legacy provider credentials; and
- extension pairing material and an unfinished active-tab session.

## Trust boundaries

| Boundary | Control | Residual risk |
| --- | --- | --- |
| React to Rust IPC | Tauri command allowlist, strict CSP, typed arguments | A compromised bundled frontend could invoke exposed commands |
| Rust to SQLite | One application writer, parameterized queries, versioned migrations | SQLite is not application-level encrypted |
| Raw events to evidence | Local deterministic rules, sanitization, provenance, confidence | Metadata may be sensitive and rules may be wrong |
| Human review to certification | Explicit attestation, readiness checks, canonical immutable snapshot | The user may misunderstand, be coerced, or share an account/device |
| Certification to export | Allowlisted serializer, preview, local save dialog | The user or another local process may disclose the saved file later |
| Rust to Keychain | Apple Keychain; commands never return keys | A compromised user session or permissive ACL may access keys |
| Extension to native core | Chrome origin restriction, protocol/token/ID validation, local socket | Pairing secrets are readable by a same-user compromise |
| Explicit legacy provider call | Minimized context over HTTPS | Provider processing, retention, and legal disclosure remain external |

## Pivot-specific threats

### Employer coercion

**Risk:** An employer or other party pressures a user to reveal raw activity or install Knov as a monitoring agent.

**Mitigation:** Knov offers purpose-specific certified exports and has no employer account, raw-data interface, manager mode, network streaming, or remote-control surface. The interface states that raw evidence stays local. Software cannot eliminate interpersonal or employment coercion after someone gains local access.

### Silent egress

**Risk:** Activity or derived records leave the Mac without informed action.

**Mitigation:** The business path has no automatic upload. Certification is local. Export requires preview and an explicit save action, and the preview is generated from the same allowlisted snapshot as the file. Scheduled provider refresh is disabled in the primary workflow.

### Inference error

**Risk:** Evidence is attributed to the wrong project or business category, producing misleading totals.

**Mitigation:** Inferences show high/medium/low confidence and a reason. Low confidence remains unallocated. User corrections are authoritative. Every positive-duration item must be reviewed or excluded and have resolved dimensions before certification.

### Post-certification modification

**Risk:** A record changes after the user attests to it, while continuing to appear certified.

**Mitigation:** Certification creates an immutable canonical snapshot with a SHA-256 hash. Later edits create a new draft/version and require certification again. Export reads the stored snapshot, never mutable evidence.

### Sensitive metadata leakage

**Risk:** URLs, window titles, queries, repository/path names, or source identifiers expose secrets or unrelated personal behavior.

**Mitigation:** Raw metadata remains local and expires after 30 days. Default exports allow only derived record fields and omit raw URLs, titles, paths, event IDs, application timelines, excluded/personal evidence, prompts, and credentials.

### Shared account or device

**Risk:** A collected event is attributed to the wrong human because another person used the Mac or account.

**Mitigation:** Knov treats device activity as evidence, not forensic identity proof. Review supports exclusion, personal/non-work marking, and uncertainty. Certification is a human attestation “to the best of my knowledge,” with no claim that every event was produced by that person.

## Existing controls

- Collection is user-controlled and supports pause, application/domain exclusions, selected browser profiles, and deletion.
- The extension has no content scripts, ignores incognito/non-HTTP(S) pages, and does not retain completed delivery queues.
- Native messages are bounded and authenticated; the local socket is per-user.
- Event fingerprints and interval handling prevent duplicate ingestion and double-counting.
- Editor collection avoids source contents, snapshots, hidden/generated/dependency trees, and credential-like paths.
- Provider status errors do not echo credentials or raw response bodies.
- The local audit trail records lifecycle metadata without copying sensitive evidence content.
- Certified snapshots survive raw retention without embedding the raw history.

## Material residual risks

Window and page titles remain difficult to sanitize perfectly. Exact exclusion rules can miss renamed apps, novel subdomains, or sensitive text in otherwise permitted contexts. Local SQLite/WAL data, backups, and pairing state remain available to sufficiently privileged local actors.

Integrity hashes detect changes to the canonical snapshot; they do not prove who certified it, when an independent timestamp authority observed it, or whether the underlying evidence was complete. CSV/JSON files can be copied or edited after export. Recipients need the stored certification metadata to compare hashes, and the alpha does not provide a hosted verifier.

The optional extension expands browser permissions and local attack surface. The loopback fallback has no TLS and is development-only. Source builds are unsigned and unnotarized, and there has been no independent penetration test.

## Recommended alpha posture

- Use Knov only on a personally controlled test Mac with FileVault and a strong login.
- Exclude sensitive applications and domains before resuming collection.
- Review every included interval; treat suggested classifications as provisional.
- Inspect the export preview and destination before saving or sending a record.
- Use limited-purpose provider keys only when deliberately testing legacy features.
- Do not describe the R&D demo as compliant, audit-proof, or professional advice.
