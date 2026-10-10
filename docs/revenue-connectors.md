# Revenue source setup

Connectors perform read-only, explicitly requested imports into a selected real project. They never send email, post Slack messages, import an entire Slack workspace, or run instructions found in source content. Demo projects cannot receive live connector data. Credentials, account identity, approved range/query/channels, and last-sync time are held in macOS Keychain (`com.knov.revenue.connectors`), separate from AI provider credentials. Secrets are never returned by IPC.

## Gmail

1. Enable Gmail API in a Google Cloud project, configure its OAuth consent screen, and create an OAuth client of type **Desktop app**. Add your account as a test user if the consent screen is in testing. Public distribution of `gmail.readonly` may require Google's restricted-scope verification.
2. Enter the Desktop client ID (`installed.client_id` in downloaded Google Desktop credentials JSON) and optionally its matching `installed.client_secret` into the password field, then a specific Gmail query (for example `from:client@example.com`), explicit start/end dates (the UI uses midnight UTC boundaries), and an import limit of 1–100 messages. Confirm authorization and use the Gmail authorization control. Knov opens the system browser with account selection and consent, a random IPv4 loopback port, state validation, and S256 PKCE. No credentials are embedded in the app.
3. Alternatively supply an already authorized OAuth access token issued with `https://www.googleapis.com/auth/gmail.readonly`. Knov verifies token scopes against Google and retrieves the actual account identity. Broader Gmail/mail scopes are rejected. Only selected query and range are read.
4. Trigger Sync for the chosen project. Imports include subject, From/To/Cc, date, thread/message IDs and plain-text MIME bodies. Attachments, HTML-only bodies, external linked pages and tracking content are not fetched.

The current desktop flow is macOS-only and uses access tokens without storing a refresh token. Reauthorize when a token expires; unattended continuous sync and Gmail history cursors are not implemented. Google’s native-app documentation marks `client_secret` optional at the code-exchange endpoint. Supply the matching Desktop credential secret when needed by your client configuration; it is sent only in the HTTPS code-exchange form, is bounded to 8,192 bytes, and is never put in the browser URL, saved locally, logged or returned by IPC. Failed code exchange is reported explicitly. No Google credentials were available in the development environment, so real account authorization and API retrieval have not been live-tested.

References: [Google installed-app OAuth and PKCE](https://developers.google.com/identity/protocols/oauth2/native-app), [Gmail list/query API](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/list), [Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes).

## Slack

Create a Slack app and install it into the authorized workspace via Slack OAuth. Configure only `channels:history` and optionally `channels:read` for public channels; add `groups:history` and optionally `groups:read` only if private channels are needed. Use the resulting authorized access token in Knov. Knov verifies it with `auth.test`, displays the returned workspace/user IDs, and rejects missing scope headers, direct-message scopes, write scopes, and other broader scopes. Bots must be members of the selected channels.

Explicitly select 1–10 authorized channel IDs starting with `C` or `G`, a time range, and a total message limit of 1–100. No channel enumeration, discovery, joining, or automatic workspace import occurs. Sync retrieves channel history with participant IDs, timestamps and parent-thread IDs. For discovered parent messages with replies, sync retrieves bounded `conversations.replies` pages in the same selected channel and time range. Parent messages older than the approved range are not discovered, so their recent replies may be absent; this is a scoped import, not a complete workspace or thread archive. Public/private channel reply retrieval generally requires an authorized user token; bot tokens may be rejected by Slack. Slack enforces token-type, membership, app-distribution and rate limits; permission/rate-limit failures are visible and no automatic retries broaden the scope.

Slack's browser authorization-code exchange is not embedded in this desktop MVP: the operator must configure and complete Slack's app installation/OAuth outside Knov, then provide the issued token. A production in-app Slack flow needs a registered HTTPS redirect and an exchange service that protects the app client secret. This is a real read-only API adapter with manual OAuth-token provisioning, not a mock connected account. No live Slack credentials were available for validation.

References: [Slack history](https://docs.slack.dev/reference/methods/conversations.history/), [auth.test](https://docs.slack.dev/reference/methods/auth.test/), [auth.revoke](https://docs.slack.dev/reference/methods/auth.revoke/).

## Bounds, retention and disconnect

Each sync uses at most ten pages per selected source/channel and ten pages per discovered thread, a total configured message count, a 20-second HTTP timeout per request, a 2 MB response limit, and 64,000 UTF-8 bytes per imported message, truncated only at complete character boundaries. The time range cannot exceed 366 days. The result explicitly reports truncation. Repeated imports use stable provider/message references so the revenue store can deduplicate them. Each fetched batch is ingested in one database transaction; a later invalid record rolls back earlier inserts and updates in that batch. Sync is manual; the range is fixed and last-sync is informational, not an incremental watermark.

Disconnect immediately deletes the local Keychain entry. Choosing revoke also calls the provider revocation endpoint; remote failure is reported after local disconnection, with provider account settings as recovery. Deleting imported Gmail/Slack evidence invalidates any sync already in flight under the same source lock before database deletion; a later explicitly initiated sync may import the approved range again using the retained credentials. Disconnect does not delete already imported project evidence. Use revenue evidence deletion or full-data deletion for persisted content; existing retention controls prune retained revenue records. Full-data deletion also removes both connector Keychain entries, but does not remotely revoke provider authorization. Revenue source imports remain local; this connector module makes no AI model requests.

## Contract import

User-selected `.txt`, `.md`, `.markdown` and text-based `.pdf` files up to 2 MB are read locally. Extraction returns text for review before agreement persistence. PDF extraction calls the separately installed Poppler `pdftotext` executable with argument boundaries (no shell). Install on macOS with `brew install poppler`; `pdftotext` was unavailable on the development host. PDF execution times out after 15 seconds and extracted output is capped at 2 MB. Missing extractor, extraction errors, empty/scanned PDF and unsupported formats produce explicit errors. No OCR or fallback invented text is supplied.

## IPC contracts

- `revenue_connector_status({provider})` → `{provider, connected, account, scope, lastSync}`; provider is `gmail` or `slack`.
- `revenue_connector_connect({provider, token, scope, authorized:true})` → status. Scope is `{query, after, before, channels, maxItems}`; times are Unix seconds.
- `revenue_gmail_oauth({clientId, clientSecret?:string, scope, authorized:true})` → status after browser consent, with a 180-second consent timeout.
- `revenue_connector_sync({provider, projectId})` → `{imported, truncated}`.
- `revenue_connector_disconnect({provider, revoke})` → void (or explicit remote-revocation failure after local disconnection).
- `revenue_extract_document({path, authorized:true})` → `{title, text}`. Pass reviewed text into `revenue_import_agreement`.

Focused offline tests cover explicit authorization, empty/oversized scope, selected channel IDs, rejection of write permissions and direct messages, literal prompt-injection text, and document validation. Live authorization, remote revocation and actual Keychain behavior require manual verification on an authorized macOS account. Core revenue tests cover retained evidence deletion and retention separately.
