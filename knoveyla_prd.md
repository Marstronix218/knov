# Knoveyla — Software Requirements Document

**A behavioral context layer for personal AI**

Version 0.9 — Draft · Technical-user alpha scope
Architecture: native Apple Silicon macOS collector + Chrome extension + chat interface

---

## Contents

1. [Overview](#1-overview)
2. [Users and context of use](#2-users-and-context-of-use)
3. [System architecture](#3-system-architecture)
4. [Functional requirements](#4-functional-requirements)
5. [Data requirements](#5-data-requirements)
6. [Privacy and trust requirements](#6-privacy-and-trust-requirements)
7. [Non-functional requirements](#7-non-functional-requirements)
8. [Assumptions, dependencies, and risks](#8-assumptions-dependencies-and-risks)
9. [Success criteria](#9-success-criteria)
10. [Future directions (post-MVP)](#10-future-directions-post-mvp)

---

## 1. Overview

### 1.1 Purpose

This document specifies the requirements for Knoveyla, a personal AI system that builds an understanding of a user from their real device behavior — the applications they use and the web pages they visit — and exposes that understanding through a chat interface. The goal is an assistant that already knows the user from how they actually work and live, rather than one that must be told who they are.

This document is intended as a build specification. It defines what the MVP must do, the boundaries of that scope, and the constraints — particularly around privacy — that any implementation must respect. It is written to be handed to an engineer or small team as the basis for technical design and implementation planning.

### 1.2 Problem statement

Every AI assistant available today begins each relationship from zero. ChatGPT, Claude, and Gemini learn about a user only from what is typed into them or from a narrow set of connected accounts such as email and calendar. None of them perceive what a person actually does across their device throughout the day — which tools they live in, what they research, what they are working on right now. As a result, users repeatedly re-explain their context, and the assistance they receive is generic where it could be specific.

The behavioral signal that would close this gap — application usage, window titles, and browsing activity — exists on the user's own machine but is not captured in a lightweight, private, user-controlled way and made available to an AI assistant.

### 1.3 Hypothesis

If an AI assistant is given an accurate, continuously updated picture of a user's interests and active work — derived passively from their device behavior — then the assistance it provides will be materially more useful and require less explanation from the user, and this improvement will be large enough to change how often the user chooses this assistant over a generic one.

*The MVP exists to test this hypothesis. Success is not measured by feature completeness but by whether behavioral context demonstrably improves the assistant's usefulness and the user's retention.*

### 1.4 Solution summary

Knoveyla consists of five parts working together on the user's machine:

- **Collector.** A native background agent that records which application is in the foreground, the active window title, and the user's browser history, with timestamps and durations.
- **Chrome extension.** A narrowly permissioned companion that records the active tab's URL, title, and focus duration and sends those events only to the local Mac application.
- **Profiler.** A local process that periodically filters, redacts, and summarizes the raw activity log into a minimized digest, then uses a remote language model to generate a structured, human-readable profile of the user's interests, skills, and active projects.
- **Assistant.** A chat interface that loads the profile as context and converses with the user as an assistant that already knows them.
- **Activity dashboard.** A clear visual account of how the user spends their time, including usage percentages, a browsable activity history, and optional recommendations for useful next steps.

Collection, raw-event storage, filtering, redaction, aggregation, and profile storage happen locally on the user's device. Profile inference uses a remote language model for the MVP: only a minimized activity digest prepared locally is sent for profile generation, never the complete raw activity database. Individual chat turns and the relevant local profile context are also sent to the selected language model provider when the user actively converses. These egress paths must be disclosed clearly before collection begins.

### 1.5 Scope and non-goals

The following table sets the boundary of the MVP. Items marked out of scope are deliberately excluded from the first version and are not to be built unless re-prioritized.

| In scope (MVP) | Out of scope (MVP) |
|---|---|
| Foreground app + window title capture | Reading inside other apps' content |
| Apple Silicon application targeting macOS 26 | Intel Macs, Windows, and Linux |
| Local browser history ingestion | Screen recording or audio capture |
| Chrome extension for active-tab URL, title, and duration | Browser page-body or DOM capture |
| Best-effort Safari and Firefox support when low-cost | Safari/Firefox work that delays required Chrome support |
| Local activity database | Mobile (iOS / Android) clients |
| Periodic profile generation via LLM | Social media in-app content (no API access) |
| Chat interface using the profile | Email, Spotify, and other OAuth sources |
| Activity dashboard with usage percentages and history | Background or push notifications |
| In-app recommendations for useful next steps | Autonomous actions taken on the user's behalf |
| Selection of one or more Chrome profiles during onboarding | Automatic ingestion from Chrome profiles the user did not select |
| Bring-your-own OpenAI or Anthropic API key | Knoveyla-hosted LLM proxy, billing, or user accounts |
| User view + edit + delete of profile | Proactive notifications / distraction coaching |
| Single device, single user | Multi-device sync; cloud accounts |

**Note on recommendations and notifications.** Contextual next-step recommendations are included inside the activity dashboard and assistant experience. Background, push, or interruptive goal-aware nudges ("you said you wanted to finish X, you've been on Y for 20 minutes") remain explicitly excluded from the MVP. They introduce a notification-fatigue problem that should not be tackled until the core hypothesis is validated.

---

## 2. Users and context of use

### 2.1 Target user

The primary user of the alpha is a technically capable, AI-heavy knowledge worker using an Apple Silicon Mac running macOS 26: a developer, technical freelancer, researcher, or similar individual who already uses AI assistants daily and is frustrated at having to re-establish context each time. This user runs multiple parallel projects and interests, is comfortable installing development-stage desktop software and a manually distributed browser extension, already has or can obtain an OpenAI or Anthropic API key, and is willing to grant system permissions in exchange for clear value. They are privacy-aware and will reject a product that feels like surveillance.

### 2.2 Why this user first

This user is reachable, feels the problem acutely, and can evaluate the result. They are also tolerant of the installation friction a native collector requires, which makes them suitable for validating the hypothesis before any attempt at a broader, lower-friction audience.

### 2.3 Primary use cases

1. **Contextual assistance.** The user opens the assistant and asks for help; the assistant draws on the profile to give an answer tailored to their current work without being told the context.
2. **Being recognized.** On opening the assistant, the user is greeted with an accurate, concise summary of what they have been focused on, creating the sense of being understood.
3. **Profile inspection.** The user opens their profile to see what the system has inferred about them, and edits or removes anything inaccurate or unwanted.
4. **Control and deletion.** The user pauses collection, excludes specific apps or sites, or deletes all collected data at any time.
5. **Activity review.** The user opens a dashboard to see time allocation and usage percentages by application or site and to browse a chronological history of what they were working on or viewing.
6. **Next-step guidance.** The user receives optional, non-interruptive recommendations in the app based on recent activity and the current profile, such as a relevant task to resume or a logical next action.
7. **Chrome profile consent.** During onboarding, the user sees the Chrome profiles available on the device and explicitly selects one or more profiles whose history Knoveyla may ingest.
8. **Technical alpha setup.** A technical user installs a development-stage Mac build, loads or installs the companion Chrome extension, grants permissions, selects Chrome profiles, and connects their own LLM provider key.
9. **Provider configuration.** The user selects OpenAI or Anthropic, enters an API key in the app, verifies the connection, and has the credential stored in macOS Keychain rather than in the activity database or a required `.env` file.

---

## 3. System architecture

### 3.1 Why native, not web

A web application runs inside the browser security sandbox and cannot read browser history files, query operating-system foreground-window APIs, see other applications, or run in the background after its tab is closed. Every behavioral signal central to this product is therefore invisible to a pure web app. The collector must be native.

This constraint is also strategically favorable: because the data requires a native agent with system permissions, the capability is not trivially replicable by a website, which contributes to the product's defensibility.

### 3.2 Component responsibilities

| Component | Responsibility | Runtime |
|---|---|---|
| Collector | Capture foreground app, window title, browser history; write to local store | Native background agent |
| Chrome extension | Capture active-tab URL, title, and focus duration without reading page content; forward events to the local collector | Companion Chrome extension; unpacked installation permitted for alpha |
| Local store | Persist raw activity events and the generated profile | Embedded SQLite database |
| Profiler | Filter and minimize raw activity locally, send a bounded digest to a remote LLM, and store the resulting structured profile locally | Local process invoking a remote LLM |
| Assistant | Chat UI; loads profile as context; sends turns to LLM | Web or native UI on local data |
| Activity dashboard | Show usage percentages, time allocation, chronological history, and in-app next-step recommendations | Part of the assistant UI |
| Control panel | View/edit/delete profile; manage collection settings and permitted Chrome profiles | Part of the assistant UI |

### 3.3 Recommended starting shape

The application uses Tauri 2: React and TypeScript bundled with Vite for the interface, a Rust core for collection and local orchestration, and embedded SQLite for persistent data. A narrowly scoped Swift helper may be added only for macOS APIs that cannot be implemented reliably through the Rust layer. The alpha is built and tested for the `arm64` architecture and macOS 26; Intel (`x86_64`) and universal binaries are not required. The deployment target may be lowered to support older Apple Silicon macOS releases only when the chosen dependencies and APIs work without a separate compatibility implementation, meaningful additional testing burden, or delay to the macOS 26 alpha. A companion TypeScript Chrome extension provides accurate active-tab metadata and communicates only with the local Mac application.

The technical-user alpha may use a development-stage Mac build and an unpacked Chrome extension installed through browser developer mode. Chrome Web Store publication, polished installers, Apple notarization, and consumer-grade installation are deferred until the behavioral-context hypothesis is validated. Setup documentation must nevertheless be explicit, reproducible, and honest about every permission.

Knoveyla uses a bring-your-own-key model for the alpha. Users select OpenAI or Anthropic and enter their API key in the application settings. The packaged application stores the credential in macOS Keychain and uses it only for direct requests to the selected provider. A `.env` file may be supported for contributors running the source code, but it must not be required for normal alpha use.

### 3.4 Data flow

1. During setup, the technical user installs or loads the Chrome extension, grants required macOS permissions, and explicitly selects one or more detected Chrome profiles. Up to 90 days of browser history is imported only from the selected profiles into a temporary cold-start dataset.
2. The user selects OpenAI or Anthropic, enters an API key in the Mac application, and verifies the connection. The application stores the key in macOS Keychain.
3. The collector samples the foreground application and active window title at a fixed interval and records permitted browser-history additions, writing timestamped events to the local store.
4. While Chrome is in use, the extension records active-tab URL, title, focus start time, and duration. It sends these metadata events only to the local Mac collector and does not read page content.
5. The local collector deduplicates and reconciles live extension events with imported browser history.
6. For the first profile only, the profiler uses the temporary 90-day browser dataset to improve cold-start topic and project inference.
7. After the first profile is generated successfully, imported detailed events older than the normal 30-day window are permanently deleted. The temporary bootstrap dataset must never become an ongoing 90-day retention policy.
8. The dashboard queries the local store to show usage percentages, time allocation, and chronological activity history.
9. On a schedule (for example nightly), the profiler reads recent events and locally filters, redacts, deduplicates, and aggregates them into a minimized activity digest.
10. Using the user's Keychain-backed API key, the minimized digest is transmitted directly to the configured remote LLM, which returns an updated structured profile and optional next-step recommendations. The generated profile and recommendations are stored locally; the request digest is ephemeral and discarded after the response is processed. The complete raw activity log is never transmitted.
11. When the user opens the assistant, the current profile is loaded and supplied to the LLM as context for the conversation.
12. Each chat turn the user sends is transmitted directly to the selected LLM provider using the user's API key, together with the relevant profile context; the response is returned to the UI.

---

## 4. Functional requirements

Requirements are identified as FR-n. Priority is Must (required for MVP), Should (valuable, include if feasible), or Could (defer if needed).

### 4.1 Collector

| ID | Requirement | Priority |
|---|---|---|
| FR-1 | Capture the current foreground application name at a configurable sampling interval. | Must |
| FR-2 | Capture the active window title alongside the app name. | Must |
| FR-3 | Record start time and duration for each continuous app-focus session. | Must |
| FR-4 | On first setup, ingest up to the previous 90 days of browser history (URL, page title, visit time) from the Chrome profile or profiles explicitly selected by the user for cold-start profile generation. | Must |
| FR-5 | Run continuously in the background and resume automatically after reboot. | Must |
| FR-6 | Respect an exclusion list of apps and domains the user does not want recorded. | Must |
| FR-7 | Capture extracted search-query terms from browser history where present in the URL. | Should |
| FR-8 | Detect available Chrome profiles and ingest data only from the one or more profiles the user authorized. | Must |
| FR-9 | Support Safari and Firefox on a best-effort basis only when doing so introduces no separate substantial implementation path and no delay to required Chrome support. | Should |

### 4.2 Profiler

| ID | Requirement | Priority |
|---|---|---|
| FR-10 | Map raw app and URL activity into interest and topic categories. | Must |
| FR-11 | Generate a structured, human-readable profile (interests, skills, active projects, patterns). | Must |
| FR-12 | Regenerate the profile on a schedule by sending a minimized, locally prepared activity digest to a remote language model. | Must |
| FR-13 | Filter, redact, deduplicate, and aggregate recent activity locally before remote profile generation; never send the complete raw activity database. | Must |
| FR-14 | Express inferences with appropriate uncertainty; avoid stating weak guesses as fact. | Must |
| FR-15 | Store the profile in a format the user can read and edit directly. | Must |
| FR-16 | Preserve user edits to the profile across regenerations rather than overwriting them. | Should |
| FR-17 | Retrieve only the relevant portion of a large profile for a given query (deferred until profile size requires it). | Could |

### 4.3 Assistant

| ID | Requirement | Priority |
|---|---|---|
| FR-18 | Provide a chat interface backed by a remote language model (e.g. Claude or GPT via API). | Must |
| FR-19 | Load the current profile as context for every conversation. | Must |
| FR-20 | Open a new session with a concise, accurate summary of the user's recent focus. | Must |
| FR-21 | Reference profile knowledge naturally, without restating raw logs or feeling intrusive. | Must |
| FR-22 | Allow the user to correct the assistant when an inference is wrong, and reflect that correction. | Should |

### 4.4 Control and transparency

| ID | Requirement | Priority |
|---|---|---|
| FR-23 | Let the user view the full profile at any time. | Must |
| FR-24 | Let the user edit or delete any part of the profile. | Must |
| FR-25 | Let the user pause and resume collection. | Must |
| FR-26 | Let the user delete all collected data permanently in a single action. | Must |
| FR-27 | Let the user manage the app/domain exclusion list from the UI. | Must |
| FR-28 | Show the user clearly what is collected, what remains local, what is sent to the remote LLM, and where local data is stored. | Must |
| FR-29 | During onboarding, show the detected Chrome profiles and require the user to select one or more before any browser-history ingestion begins. | Must |
| FR-30 | Let the user change the set of permitted Chrome profiles later from the control panel. | Must |

### 4.5 Activity dashboard and recommendations

| ID | Requirement | Priority |
|---|---|---|
| FR-31 | Show total tracked time and usage percentages by application and website for a user-selected time range. | Must |
| FR-32 | Provide a chronological, filterable history of applications, window titles, websites, and page titles captured during the selected period. | Must |
| FR-33 | Make clear which dashboard content is observed activity, which is inferred profile information, and which is a recommendation. | Must |
| FR-34 | Show optional recommendations for useful next steps based on recent activity and the current profile. | Must |
| FR-35 | Let the user dismiss a recommendation and provide feedback when it is irrelevant or unwanted. | Should |
| FR-36 | Keep recommendations inside the app for the MVP; do not send background, push, or interruptive notifications. | Must |

### 4.6 Chrome extension and browser activity

| ID | Requirement | Priority |
|---|---|---|
| FR-37 | Capture the active Chrome tab's URL, title, focus start time, and focus duration while Chrome is the foreground application. | Must |
| FR-38 | Use browser tab metadata only; do not inject content scripts, inspect the DOM, read page bodies, capture form input, take screenshots, or record keystrokes. | Must |
| FR-39 | Send extension events only to the paired local Mac application through an authenticated local communication channel. | Must |
| FR-40 | Apply collection pause and domain exclusions to extension events immediately. | Must |
| FR-41 | Show the extension's connection and collection state in both the extension UI and the Mac application. | Must |
| FR-42 | Detect when the extension is missing, disconnected, or lacks permission and explain that live browser-duration accuracy is degraded while history import remains available. | Must |
| FR-43 | Provide documented setup and verify successful pairing for an unpacked or manually distributed Chrome extension during the technical alpha. | Must |

### 4.7 LLM provider credentials

| ID | Requirement | Priority |
|---|---|---|
| FR-44 | Let the user select a supported remote LLM provider, initially OpenAI or Anthropic. | Must |
| FR-45 | Let the user enter, replace, validate, and remove their provider API key from the application settings. | Must |
| FR-46 | Store provider API keys in macOS Keychain; never store them in the activity database, logs, analytics, Chrome extension, source control, or plaintext application configuration. | Must |
| FR-47 | Send profiling and chat requests directly from the Mac application to the selected provider; do not route them through a Knoveyla-hosted proxy in the alpha. | Must |
| FR-48 | Support `.env`-based credentials only as an optional development convenience for contributors running from source, never as the required alpha-user setup. | Should |
| FR-49 | Show actionable errors for missing, invalid, revoked, rate-limited, or out-of-credit provider keys without exposing the credential. | Must |

### 4.8 Platform support

| ID | Requirement | Priority |
|---|---|---|
| FR-50 | Build and run the alpha natively on Apple Silicon (`arm64`) Macs running macOS 26; Intel Mac, universal-binary, Windows, and Linux support are out of scope. | Must |
| FR-51 | Support older Apple Silicon macOS releases only when doing so requires no separate compatibility code path, no material additional testing burden, and no delay to the macOS 26 alpha. | Should |

### 4.9 Retention and deletion

| ID | Requirement | Priority |
|---|---|---|
| FR-52 | Retain detailed raw activity events and detailed dashboard history in a rolling 30-day local window. | Must |
| FR-53 | Automatically and permanently purge detailed events older than 30 days from the local store. | Must |
| FR-54 | Retain the editable generated profile and preserved user corrections until the user deletes them or invokes single-action deletion of all Knoveyla data. | Must |
| FR-55 | Treat imported browser events between 31 and 90 days old as temporary cold-start data, use them for the first profile, and permanently delete them after successful initial profile generation. | Must |

### 4.10 Implementation stack

| ID | Requirement | Priority |
|---|---|---|
| FR-56 | Build the desktop application with Tauri 2 using a React and TypeScript frontend bundled with Vite. | Must |
| FR-57 | Implement collection, local orchestration, security-sensitive operations, and native command handling in the Tauri Rust core. | Must |
| FR-58 | Use a narrowly scoped Swift helper or native bridge only when required macOS APIs cannot be implemented reliably through the Rust layer; do not create a second general-purpose application backend. | Should |
| FR-59 | Store all persistent alpha data in an embedded local SQLite database with versioned migrations. | Must |
| FR-60 | Implement the Chrome extension in TypeScript using the current Chrome extension manifest format. | Must |
| FR-61 | Do not use Next.js server-side rendering, hosted Supabase, or a local Supabase service stack in the alpha. | Must |

### 4.11 Browser support boundary

| ID | Requirement | Priority |
|---|---|---|
| FR-62 | Treat Google Chrome as the only required and release-blocking browser for profile selection, 90-day cold-start history import, and accurate live-tab timing. | Must |
| FR-63 | Import up to 90 days of Safari or Firefox history when local access, schema handling, and testing can reuse the existing browser-ingestion pipeline without material schedule impact. | Should |
| FR-64 | Add live-tab timing for Safari or Firefox only when it does not require a substantial separate extension, distribution, permission, or maintenance effort; otherwise provide history import only or omit support. | Could |
| FR-65 | Clearly label each browser's support level and distinguish imported history from accurately timed live-tab activity in setup and diagnostics. | Must |

---

## 5. Data requirements

### 5.1 Signal sources and quality

The following sources are in scope for the MVP. Signal quality varies and the design should weight sources accordingly: a raw duration on an ambiguous app carries little meaning, whereas a URL path or a descriptive window title carries a great deal.

| Source | What it yields | Signal strength | Access method |
|---|---|---|---|
| Browser history | URLs, page titles, search terms, visit times from user-selected Chrome profiles | High | Read from the selected local Chrome profile databases |
| Window titles | File names, document titles, page titles in app | High | OS foreground-window API |
| App usage | App name and time spent | Medium | OS foreground-window API |
| OS screen-time store | App durations, site categories | Medium | Local system database read |

**Design implication.** Window titles and URL paths are the richest available signal and the product's sharpest differentiator. The profiler should treat them as primary and treat bare app durations as weak, corroborating evidence only.

### 5.2 The activity event

Each captured event should record at minimum: a timestamp, the application name, the window title (where available), and for browser activity the source browser profile, URL, page title, and any extractable search terms, together with the duration of the focus session. The exact schema is left to technical design. The source browser profile is control metadata and must be visible enough for the user to understand which authorized profile produced an event.

### 5.3 The profile

The profile is the central artifact. It is a structured but human-readable summary — interests, apparent skills and expertise level, active projects, and behavioral patterns such as working rhythm. It must be small enough to supply to a language model as context and legible enough that the user can read and edit it directly. For the MVP a single editable structured document is sufficient; retrieval over a larger store is deferred until profile size demands it.

### 5.4 Retention

Detailed raw activity events and detailed dashboard history are retained locally for a rolling 30-day window. Events older than 30 days must be purged automatically and permanently during normal operation.

The first-run bootstrap may import up to 90 days of history from the browser profiles selected by the user. Imported events between 31 and 90 days old are temporary: they may be used to generate the first profile but must be deleted permanently after that profile succeeds. If initial profiling does not complete, the application must identify the temporary bootstrap state clearly and must not silently convert it into ongoing retention.

The editable generated profile and preserved user corrections remain available beyond the event window until the user deletes them. Dashboard history must be backed by the local activity store and cannot expose purged detailed events. Remote LLM requests must use an ephemeral minimized digest created for the request rather than a persistent remote copy of the raw event store. The user's single-action delete must remove all remaining raw events, temporary bootstrap data, dashboard history, profiles, corrections, recommendations, settings, and stored provider credentials.

### 5.5 Local storage technology

SQLite is the alpha's embedded local database. It stores activity events, browser-profile source metadata, profile versions, user corrections, recommendations, settings that are not secrets, and retention state. Schema changes must use versioned migrations.

Hosted Supabase is excluded because raw behavioral data and profiles are required to remain local. Running the full Supabase stack locally is also excluded because it would add a container runtime and multiple services to a lightweight background application. Supabase may be reconsidered later for explicitly opt-in account, sync, or collaboration features, but it must not become a prerequisite for the local alpha.

---

## 6. Privacy and trust requirements

Privacy is not a feature of this product; it is a precondition for it existing at all. The product asks the user to let it observe their behavior, which is among the most sensitive things software can do. A single breach of trust ends the product. These requirements therefore carry the same weight as core functionality.

### 6.1 Mandatory principles

1. **Local-first.** All raw activity data, dashboard history, generated profiles, and saved recommendations are stored on the user's device. The complete raw behavioral database is never uploaded to a server controlled by the product or to the language model provider.
2. **Minimal egress.** Two content paths may leave the device: an individual chat turn with relevant profile context when the user actively sends it, and a minimized activity digest when the product generates or refreshes the profile and recommendations. Filtering, redaction, deduplication, and aggregation occur locally before the digest is transmitted.
3. **Full transparency.** The user can at all times see what is collected, where it is stored, and what the system has inferred.
4. **Full control.** The user can pause collection, exclude sources, edit inferences, and delete everything, at any time, without friction.
5. **Explicit consent.** Collection begins only after the user has been clearly shown what will be collected, which selected Chrome profiles will be read, and what minimized information may be sent to the remote LLM. Nothing is collected silently, and unselected Chrome profiles are never ingested.

### 6.2 Framing requirement

The product must be presented to the user as an assistant getting to know them, not as a tracker monitoring them. Onboarding bears the burden of establishing this framing honestly, by showing the local-first architecture and the user's control before collection starts. This is a requirement because the same underlying behavior can read as either helpful or invasive depending entirely on framing and control, and the difference determines whether the product is adopted or uninstalled.

### 6.3 Sensitive-content caution

Behavioral data will sometimes reveal sensitive matters — health concerns, finances, personal difficulties. The profiler must avoid surfacing confident conclusions about such topics, since an incorrect or unwelcome inference ("I see you've been researching a serious illness") is a severe failure even when the underlying data is accurate. When in doubt, the system should hold back rather than presume.

### 6.4 Provider data handling

Because chat turns, relevant profile context, and minimized activity digests are sent to a third-party language model provider, the choice of provider and its data-retention terms is itself a privacy decision. The implementation should prefer API configurations that do not train on user content, minimize provider-side storage, and disable optional request persistence. The product must disclose which provider is used, what categories of information are transmitted, when transmission occurs, and what the provider's retention behavior implies. Local-model support may be considered after the MVP but is not required for the first release.

---

## 7. Non-functional requirements

| ID | Category | Requirement |
|---|---|---|
| NFR-1 | Performance | The collector must run continuously with negligible CPU and memory impact; the user should not perceive system slowdown. |
| NFR-2 | Reliability | The collector must recover automatically after sleep, reboot, or crash without losing the local store. |
| NFR-3 | Footprint | The local store must enforce the 30-day detailed-event window and must not grow without bound. |
| NFR-4 | Security | Local data must be protected at rest using the operating system's available protections. |
| NFR-5 | Usability | A technical alpha user must be able to follow documented setup, grant permissions, configure a provider key, and generate the first profile without undocumented steps. |
| NFR-6 | Transparency | Privacy-relevant state (collecting / paused, what is stored) must be visible at a glance. |
| NFR-7 | Portability | Architecture should not unnecessarily preclude later Intel Mac, Windows, or Linux support, but no cross-platform implementation work is required for the alpha. |
| NFR-8 | Distribution | The technical alpha may use an Apple Silicon-only development build and an unpacked Chrome extension; universal binaries, code signing, notarization, and Chrome Web Store publication are not alpha release gates. |
| NFR-9 | Setup | Technical-user setup must be documented and reproducible, including Mac permissions, extension installation, local pairing, Chrome-profile selection, and in-app API-key validation. |
| NFR-10 | Degraded operation | The Mac app must remain usable when the extension is unavailable and clearly distinguish imported history from accurately timed live-tab activity. |
| NFR-11 | Credential security | Provider credentials must be retrieved from macOS Keychain only when needed and must be redacted from logs, error reports, UI diagnostics, and exported data. |
| NFR-12 | OS compatibility | macOS 26 is the required and fully tested target. Any older Apple Silicon support is best-effort and must not introduce alternate product behavior or delay the required target. |
| NFR-13 | Technology footprint | Normal application use must not require Docker, a local Postgres server, a Supabase stack, Python, Node.js, or other separately installed runtimes. Required runtimes must be packaged into the application or compiled into the native bundle. |
| NFR-14 | Browser compatibility | Chrome is the release gate. Safari and Firefox compatibility is best-effort and must not delay or weaken Chrome collection, privacy controls, or verification. |

---

## 8. Assumptions, dependencies, and risks

### 8.1 Assumptions

- The target user will grant the system permissions a native collector requires, given clear value and control.
- Required alpha users have an Apple Silicon Mac running macOS 26; Intel Macs are not supported.
- Older Apple Silicon macOS releases may work when compatibility is effectively free, but they are not allowed to block or delay the required macOS 26 release.
- Window titles and browser URLs provide sufficient signal to produce a profile the user finds accurate and useful.
- A profile that fits within a language model's context window is sufficient for the MVP; retrieval over a larger store is not yet needed.

### 8.2 Dependencies

- Operating-system APIs for foreground-window and app-usage information.
- An Apple Silicon Mac running macOS 26 and an `arm64`-capable macOS development toolchain.
- Tauri 2, Rust, React, TypeScript, Vite, and embedded SQLite.
- Read access to the local browser history store.
- Chrome extension APIs for active-tab metadata and a secure local pairing/communication mechanism.
- Optional local Safari and Firefox history access when their schemas and permissions can be supported without material additional work.
- A third-party language model API (e.g. Claude or GPT) for profiling and chat.
- A user-supplied API key for a supported provider and macOS Keychain for secure credential storage.
- Local access to the history databases for the Chrome profile or profiles selected by the user.

### 8.3 Key risks

| Risk | Impact | Mitigation |
|---|---|---|
| Cold start: profile too thin in the first days to feel valuable. | High — early churn | Generate a useful profile from minimal data; set expectations during onboarding; show value quickly from browser history, which is rich immediately. |
| Incorrect or creepy inference erodes trust. | High | Express uncertainty; hold back on sensitive topics; make correction and deletion trivial. |
| Privacy perception: product reads as surveillance. | High — non-adoption | Local-first architecture, transparent onboarding, visible control, honest framing. |
| Remote profiling conflicts with user expectations of local-first behavior. | High | Explain the two egress paths before collection, minimize activity locally, never transmit the complete raw log, disclose provider retention, and let the user pause collection or delete local data. |
| The wrong Chrome profile is ingested or personal profiles are mixed unintentionally. | High | Require explicit multi-profile selection during onboarding, ingest only selected profiles, label profile sources in activity history, and allow the selection to be changed later. |
| Browser-extension permissions feel invasive or cause abandonment. | High | Request tab metadata only, prohibit page-content access, explain every captured field, provide immediate pause/exclusions, and keep all extension events local. |
| Manual extension installation or native-app pairing creates alpha setup friction. | Medium | Provide exact technical setup instructions, verify pairing in the app, and maintain a history-import degraded mode when live-tab capture is unavailable. |
| Apple Silicon-only support reduces the available tester pool. | Low for alpha | Recruit alpha testers with Apple Silicon Macs and revisit Intel or other platforms only after the core hypothesis is validated. |
| Supporting older macOS releases expands compatibility work and delays the alpha. | Medium | Treat macOS 26 as the only release gate; lower the deployment target only when the implementation and dependencies already work without separate code paths or material extra testing. |
| A mixed Rust/TypeScript stack creates maintenance overhead for a primarily web-stack developer. | Medium | Keep native commands narrow, document the frontend/native boundary, use React and TypeScript for product UI, and introduce Swift only for APIs that demonstrably require it. |
| Temporary 90-day cold-start data persists after its intended use. | High | Track bootstrap state explicitly, purge days 31–90 immediately after successful first-profile generation, expose incomplete cleanup in diagnostics, and include bootstrap data in single-action deletion. |
| Safari or Firefox support fragments browser ingestion and delays the alpha. | Medium | Keep Chrome as the only release gate; reuse the ingestion pipeline where practical and omit separate live-tracking implementations when they require meaningful additional work. |
| User-supplied API keys are mishandled or exposed. | High | Store keys only in macOS Keychain, redact credentials from all diagnostics, keep keys out of the extension and database, and send requests directly to the selected provider. |
| Provider errors, quotas, or user billing interrupt profiling and chat. | Medium | Validate keys in settings, surface actionable provider errors, preserve local collection during outages, and retry profiling only after the user resolves the provider issue. |
| Next-step recommendations feel judgmental, distracting, or incorrect. | Medium | Keep them inside the app, distinguish recommendations from observed facts, make them dismissible, and collect relevance feedback. |
| Weak signal from duration-only apps. | Medium | Weight window titles and URLs as primary; treat bare durations as corroborating only. |
| Platform incumbents add comparable behavioral context. | Medium — long term | Compete on cross-app breadth and local-first control, which incumbents are structurally less inclined to offer. |
| Permission and code-signing friction at install. | Medium | Invest in a smooth, well-explained onboarding; target a single OS first. |

---

## 9. Success criteria

Because the MVP exists to test the central hypothesis, success is defined in terms of validated learning rather than shipped features.

### 9.1 Primary signal

- After the novelty period (roughly the first week), users who have the behavioral profile continue to choose this assistant over a generic one for real tasks. Retention beyond novelty is the single most important signal.

### 9.2 Supporting signals

- Users report that the assistant's knowledge of them is accurate and that it saves them from re-explaining context.
- Users inspect their profile and find it recognizable rather than wrong or unsettling.
- Users understand their time allocation from the dashboard and find the activity history accurate.
- Users find at least some next-step recommendations relevant and useful rather than intrusive.
- Technical alpha users can follow the setup documentation, pair the extension, select Chrome profiles, enter their own provider key in the app, and reach a populated dashboard without undocumented intervention.
- The alpha runs natively on Apple Silicon macOS 26 test Macs without requiring Rosetta.
- Older Apple Silicon macOS releases are supported only when they pass the existing implementation without delaying macOS 26 delivery.
- Chrome history import and live-tab timing work as required; any Safari or Firefox support is reported accurately as best-effort and does not delay the Chrome-complete alpha.
- Users do not uninstall over privacy discomfort after seeing what is collected.

### 9.3 Explicit anti-goal

*Feature count is not a success measure. A small system that proves behavioral context changes user behavior is a success; a feature-rich system that does not is not.*

---

## 10. Future directions (post-MVP)

These are recorded to show direction. None are part of the MVP and none should influence MVP scope decisions.

- **Additional signal sources.** Email and music-service integrations via OAuth add topical and emotional signal that behavioral data alone cannot supply.
- **Proactive assistance.** Goal-aware nudges, once the interaction-design problem of avoiding notification fatigue can be addressed responsibly.
- **Context layer for other AI tools.** Exposing the profile through a standard interface so any external AI assistant can draw on it, turning the product from an app into infrastructure.
- **Consumer-ready distribution.** Signed and notarized Mac builds, Chrome Web Store publication, automatic updates, and onboarding designed for non-technical users.
- **Additional platforms and sync.** Adding Intel Mac support if justified, extending to Windows or Linux, and later synchronizing across a user's devices.

**Deliberately excluded.** Capturing in-app content from social media platforms is not a viable direction: their APIs are closed and provide no access to what the user sees or does inside them. The roadmap must not depend on it.
