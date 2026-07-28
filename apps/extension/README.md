# Knoveyla Chrome Companion

Manifest V3 extension for accurate active-tab timing. It observes only Chrome's
tab metadata APIs and sends URL, title, focus timestamps, and duration to the
paired Knoveyla app on the same computer.

## Screenshot

![Knoveyla Chrome companion settings for Native Messaging pairing](../../docs/screenshots/chrome-companion-settings.jpg)

## Build and load

```sh
npm install
npm run build
```

Open `chrome://extensions`, enable Developer mode, choose **Load unpacked**, and
select `apps/extension/dist`. The settings page opens automatically. In the
Knoveyla Mac app, create or reveal a local pairing token, then paste it into the
extension settings together with the approved Chrome profile ID shown in
Knoveyla Settings. The Mac app registers the Native Messaging host.

## Native Messaging contract

Production communication uses Chrome Native Messaging host
`com.knoveyla.companion`. Its host manifest must list the unpacked extension's
origin under `allowed_origins`. Chrome provides the four-byte length framing; the
JSON request body is:

```json
{
  "protocolVersion": 1,
  "requestId": "<UUID>",
  "extensionId": "<Chrome runtime extension ID>",
  "pairingToken": "<local pairing token>",
  "sentAt": "2026-01-01T10:00:30.000Z",
  "type": "status | events",
  "payload": {}
}
```

The host replies with one framed JSON response:

```json
{
  "protocolVersion": 1,
  "requestId": "<same UUID>",
  "ok": true,
  "acceptedEventIds": ["<event UUID>"]
}
```

For an error, `ok` is false with `errorCode` (`authentication`, `protocol`,
`unavailable`, or `internal`) and a safe user-facing `message`. An events response
may omit `acceptedEventIds` to acknowledge the complete batch, or include the
complete set of submitted IDs. `authentication` failures put the extension into
an explicit pairing-error state.

The `events` payload is:

```json
{
  "protocolVersion": 1,
  "source": "chrome_extension",
  "extensionId": "<Chrome runtime extension ID>",
  "sentAt": "2026-01-01T10:00:30.000Z",
  "events": [
    {
      "id": "<UUID>",
      "kind": "browser_focus",
      "source": "chrome_extension",
      "browser": "chrome",
      "browserProfileId": "<approved Chrome profile ID from Knoveyla Settings>",
      "url": "https://example.com/page",
      "title": "Example",
      "startedAt": "2026-01-01T10:00:00.000Z",
      "endedAt": "2026-01-01T10:00:30.000Z",
      "durationMs": 30000,
      "incognito": false
    }
  ]
}
```

Completed events are held only in service-worker memory long enough for one
delivery attempt. Failed deliveries are discarded rather than persisted or
retried, preventing stale activity from being resent after a pause or local data
deletion. Active sessions use `chrome.storage.session` and are checkpointed
approximately every 30 seconds.

### Development HTTP fallback

Settings can explicitly select local HTTP for development. Only
`http://127.0.0.1`, `http://localhost`, or `http://[::1]` are accepted. Requests
use `Authorization: Bearer <pairing-token>`, `X-Knoveyla-Protocol: 1`, and the
same status/events shapes at `GET /v1/extension/status` and
`POST /v1/extension/events`. HTTP is not the production default.

## Privacy behavior

- There are no content scripts.
- Only active-tab `url` and `title` metadata are read.
- Incognito access is disabled by the manifest.
- Non-HTTP(S), excluded, paused, unfocused, and invalid URLs are never collected.
- Completed events are never persisted by the extension.
- Collection ends as soon as Chrome loses focus, the active page changes, the
  extension is paused, or a matching domain exclusion is saved.
- The pairing token is stored only in Chrome local extension storage and sent
  only through Native Messaging (or the explicitly selected loopback fallback).

## Verification

```sh
npm run typecheck
npm test
npm run build
```
