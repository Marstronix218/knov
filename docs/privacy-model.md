# Privacy model

Knov is local-first, not fully local. Raw activity is stored on the Mac, but
profile generation, recommendations, prediction generation, connection tests,
and chat use the user-selected OpenAI, Anthropic, or Amazon Bedrock API.

## What is collected

The desktop collector can record:

- foreground application name
- focused window title when Accessibility permission is available
- session timestamps and duration
- selected Chrome-history URL, page title, visit time, and recognized search
  query
- metadata-only save signals and recent safe Git working-tree paths from
  supported editor workspaces

The optional experimental Chrome extension can record the focused tab's URL,
title, start/end time, duration, and extension ID. It ignores incognito tabs,
non-HTTP(S) URLs, and locally excluded domains. The baseline MVP does not
require the extension; selected Chrome history and foreground app/window data
come from the desktop app.

Knov does not intentionally collect page bodies, DOM content, form input,
keystrokes, clipboard contents, screenshots, audio, or camera data. The Chrome
extension has no content scripts. Editor collection does not open source files
or saved Local History snapshots and excludes hidden, generated, credential,
certificate, and dependency paths.

Window titles, page titles, URLs, and search queries can nevertheless contain
sensitive information. Treat the local database as sensitive.

## What remains local

The following remains in app-owned local storage unless the user exports or
copies it outside Knov:

- detailed activity events and dashboard history
- complete imported URLs, titles, and extracted search queries
- generated profile versions, recommendations, and corrections
- prediction candidates, sanitized state summaries, inferred goals, observed
  outcomes, evaluation scores, and optional feedback
- work-agent data: learned workflows (step labels such as app names and
  domains, timing statistics, and recent occurrences with URL paths but no
  queries or titles), your workflow and goal reviews, skills, permissions,
  approved project folders, 10-minute state snapshots, and the action journal
  (plans, targets, rationale, decisions, verification results, and trimmed
  command output)
- agent drafts, written as Markdown files in Knov's own `drafts` folder
- settings and Chrome pairing state
- allowlisted alpha outcome events such as setup completion, thread resume/copy,
  and useful/wrong/not-now feedback; these contain only an event type, local
  thread identifier, and timestamp

Provider keys are stored separately in macOS Keychain. The Chrome pairing token
is stored in SQLite and in the extension's local Chrome storage; it is not a
provider credential.

## What leaves the Mac

| Action | Data sent directly to provider |
| --- | --- |
| OpenAI connection test | API key in authorization; request to list models |
| Anthropic connection test | API key plus a minimal `Reply OK` message |
| Amazon Bedrock connection test | API key plus a minimal model-specific token-count request |
| Profile refresh | Aggregated activity digest and all authoritative corrections |
| Prediction generation (only with a provider key) | Minimized current-work features and a small set of sanitized historical patterns needed to produce candidates |
| Work agent | Nothing. Workflow mining, goals, planning, permissions, verification, and the journal are local |
| Chat | Locally retrieved profile facts, query-specific aggregates, bounded conversation, new message, and sanitized evidence from the explicitly selected thread |
| Workflow interview analysis | Bounded interview answers and workflow state, plus minimized metadata from the explicitly selected thread |

The profiling digest includes app names, domain-only website identifiers,
durations, counts, and window/page-title strings truncated to 180 characters.
Local redaction removes common credential markers, email-shaped identifiers,
home-directory paths, and long token-like identifiers before truncation. It is
not a general sensitive-data detector, so a title may still disclose private
information.

Knov does not send the complete activity-events table or complete URLs as
part of the profile digest. Chat context excludes URL queries/fragments, local
absolute paths, identifiers, and credential-like values. The full comparison
baseline and inference-run economics remain local.
Requests go from the Rust core directly to the selected provider; there is no
Knov proxy or analytics service. OpenAI requests set `store: false`.
Provider-side processing and retention remain governed by the selected
provider's API terms and account settings.

Workflow Discovery persists its interview transcript, reviewed workflow, and
graph revision history locally, unlike ordinary chat. Answers may contain
business information: submitting an answer for AI analysis sends bounded
interview context directly to the selected provider. Skip, pause, review, and
local corrections do not grant execution permissions. Model-derived facts
remain hypotheses until explicitly reviewed. Removing an interview and
**Delete everything** remove its app-owned workflow and graph history;
this does not erase copies already processed by a provider.

Prediction requests do not contain complete browsing history, full URLs,
absolute local paths, credentials, excluded activity, or unrelated historical
events. The raw provider prompt is not stored. Historical retrieval, the
heuristic baseline, outcome evaluation, aggregate accuracy metrics, and user
feedback remain local. Disabling the experiment stops new prediction requests;
pausing collection also suppresses prediction generation.

The work agent has no network path of its own. Its actions run on this Mac:
it can open a credential-free web page or an application you have used in the
workflow, save a draft to its own folder, or run one of eight fixed test
commands (no shell, no custom arguments) inside a project folder you approved.
Test commands are your project's own code and may themselves contact the
network, as they would when you run them. Their output is trimmed, has the home
path and credential-looking lines removed, and stays in SQLite. Pausing the
agent stops all of this independently of collection.

Rendering activity history does not contact recorded websites. Knov uses local
application icons or letter placeholders, and resource previews remain
metadata-only links until the user explicitly opens a resource.

## Credentials

The settings and onboarding interfaces pass a newly entered key to a Rust
command, which saves it to Keychain service `com.knov.desktop.llm`. Commands
never return the key to the frontend.

For source development only, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or
`AWS_BEDROCK_API_KEY` in the native process environment takes precedence over Keychain after a provider has
been configured. Environment variables do not configure first-run provider
selection, and Knov does not load `.env` files automatically. Environment
variables may be visible to other processes with sufficient local privileges
and should not be used for a distributed alpha build.

## Retention

- Normal detailed activity is retained for a rolling 30 days.
- Imported events from days 31–90 are temporary bootstrap data.
- Temporary bootstrap data is deleted only after the first profile refresh
  succeeds. A failed or unavailable provider leaves it in place for retry.
- Profiles and corrections remain until removed through the app's controls.
- Predictions, evaluations, and feedback remain local until removed through
  **Delete everything**.
- Agent state snapshots follow the 30-day activity window. Agent runs and the
  action journal are kept for 90 days as an audit trail. Workflow evidence is
  recomputed from the 30-day window; workflows you confirmed or dismissed keep
  your decision but lose evidence once they stop recurring. Skills,
  permissions, approved folders, goal reviews, and drafts remain until you
  remove them or use **Delete everything**.
- If installed, the extension does not persist completed activity events. An
  unfinished active session may exist in Chrome session storage.

Expired normal activity is purged while the app is running, including while
collection is paused. If the app is not running, purge execution is delayed
until the next launch.

## Pause, exclusions, and deletion

Desktop collection starts disabled and remains disabled until the user resumes
it from the app. Desktop app exclusions are enforced by the Rust collector and
ingestion core. If the optional extension is installed, its exclusions and
pause state are enforced by the extension.

The extension checks the desktop collection state before delivery and on its
regular checkpoint. Events completed during a stale-policy window are discarded,
not retained for later upload. Configure domain exclusions in both places when
testing the extension.

`Delete everything`:

- removes app-owned SQLite rows, including agent workflows, skills, runs,
  permissions, approved folders, goal reviews, and snapshots
- deletes Knov's agent `drafts` folder
- resets settings to defaults
- removes all configured provider credentials from Keychain or reports failure
- creates a new pairing token
- removes Knov's per-user Chrome Native Messaging manifest

It does not promise forensic or cryptographic erasure. SQLite/WAL pages, APFS
snapshots, backups, SSD behavior, crash remnants, and provider-held request data
are outside that guarantee. The database file and Chrome extension storage are
not removed by the in-app action. Clear the
extension's site data or remove the extension to delete its pairing
configuration.
