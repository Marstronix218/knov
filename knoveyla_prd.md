# Knoveyla — Software Requirements Document

**A behavioral context layer for personal AI**

Version 0.2 — Draft · MVP scope
Architecture: native collector + chat interface

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

Knoveyla consists of four parts working together on the user's machine:

- **Collector.** A native background agent that records which application is in the foreground, the active window title, and the user's browser history, with timestamps and durations.
- **Profiler.** A local process that periodically filters, redacts, and summarizes the raw activity log into a minimized digest, then uses a remote language model to generate a structured, human-readable profile of the user's interests, skills, and active projects.
- **Assistant.** A chat interface that loads the profile as context and converses with the user as an assistant that already knows them.
- **Activity dashboard.** A clear visual account of how the user spends their time, including usage percentages, a browsable activity history, and optional recommendations for useful next steps.

Collection, raw-event storage, filtering, redaction, aggregation, and profile storage happen locally on the user's device. Profile inference uses a remote language model for the MVP: only a minimized activity digest prepared locally is sent for profile generation, never the complete raw activity database. Individual chat turns and the relevant local profile context are also sent to the selected language model provider when the user actively converses. These egress paths must be disclosed clearly before collection begins.

### 1.5 Scope and non-goals

The following table sets the boundary of the MVP. Items marked out of scope are deliberately excluded from the first version and are not to be built unless re-prioritized.

| In scope (MVP) | Out of scope (MVP) |
|---|---|
| Foreground app + window title capture | Reading inside other apps' content |
| Local browser history ingestion | Screen recording or audio capture |
| Local activity database | Mobile (iOS / Android) clients |
| Periodic profile generation via LLM | Social media in-app content (no API access) |
| Chat interface using the profile | Email, Spotify, and other OAuth sources |
| Activity dashboard with usage percentages and history | Background or push notifications |
| In-app recommendations for useful next steps | Autonomous actions taken on the user's behalf |
| Selection of one or more Chrome profiles during onboarding | Automatic ingestion from Chrome profiles the user did not select |
| User view + edit + delete of profile | Proactive notifications / distraction coaching |
| Single device, single user | Multi-device sync; cloud accounts |

**Note on recommendations and notifications.** Contextual next-step recommendations are included inside the activity dashboard and assistant experience. Background, push, or interruptive goal-aware nudges ("you said you wanted to finish X, you've been on Y for 20 minutes") remain explicitly excluded from the MVP. They introduce a notification-fatigue problem that should not be tackled until the core hypothesis is validated.

---

## 2. Users and context of use

### 2.1 Target user

The primary user of the MVP is an AI-heavy knowledge worker: a freelancer, developer, researcher, or similar individual who already uses AI assistants daily and is frustrated at having to re-establish context each time. This user runs multiple parallel projects and interests, is comfortable installing desktop software, and is willing to grant system permissions in exchange for clear value. They are privacy-aware and will reject a product that feels like surveillance.

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

---

## 3. System architecture

### 3.1 Why native, not web

A web application runs inside the browser security sandbox and cannot read browser history files, query operating-system foreground-window APIs, see other applications, or run in the background after its tab is closed. Every behavioral signal central to this product is therefore invisible to a pure web app. The collector must be native.

This constraint is also strategically favorable: because the data requires a native agent with system permissions, the capability is not trivially replicable by a website, which contributes to the product's defensibility.

### 3.2 Component responsibilities

| Component | Responsibility | Runtime |
|---|---|---|
| Collector | Capture foreground app, window title, browser history; write to local store | Native background agent |
| Local store | Persist raw activity events and the generated profile | Local database (e.g. SQLite) |
| Profiler | Filter and minimize raw activity locally, send a bounded digest to a remote LLM, and store the resulting structured profile locally | Local process invoking a remote LLM |
| Assistant | Chat UI; loads profile as context; sends turns to LLM | Web or native UI on local data |
| Activity dashboard | Show usage percentages, time allocation, chronological history, and in-app next-step recommendations | Part of the assistant UI |
| Control panel | View/edit/delete profile; manage collection settings and permitted Chrome profiles | Part of the assistant UI |

### 3.3 Recommended starting shape

The collector is implemented as a lightweight native agent (for example Electron, Tauri, or a small Swift/Rust/Python background process). The chat interface may be delivered as a local web UI hosted by the agent, which keeps interface development fast while preserving native data access underneath. The split is: native for collection, flexible for interface, local for storage.

### 3.4 Data flow

1. During onboarding, the user grants required system permissions and explicitly selects one or more detected Chrome profiles. Browser history is ingested only from the selected profiles.
2. The collector samples the foreground application and active window title at a fixed interval and records permitted browser-history additions, writing timestamped events to the local store.
3. The dashboard queries the local store to show usage percentages, time allocation, and chronological activity history.
4. On a schedule (for example nightly), the profiler reads recent events and locally filters, redacts, deduplicates, and aggregates them into a minimized activity digest.
5. The minimized digest is transmitted to the configured remote LLM, which returns an updated structured profile and optional next-step recommendations. The generated profile and recommendations are stored locally; the request digest is ephemeral and discarded after the response is processed. The complete raw activity log is never transmitted.
6. When the user opens the assistant, the current profile is loaded and supplied to the LLM as context for the conversation.
7. Each chat turn the user sends is transmitted to the LLM provider together with the relevant profile context; the response is returned to the UI.

---

## 4. Functional requirements

Requirements are identified as FR-n. Priority is Must (required for MVP), Should (valuable, include if feasible), or Could (defer if needed).

### 4.1 Collector

| ID | Requirement | Priority |
|---|---|---|
| FR-1 | Capture the current foreground application name at a configurable sampling interval. | Must |
| FR-2 | Capture the active window title alongside the app name. | Must |
| FR-3 | Record start time and duration for each continuous app-focus session. | Must |
| FR-4 | Ingest browser history (URL, page title, visit time) from the Chrome profile or profiles explicitly selected by the user. | Must |
| FR-5 | Run continuously in the background and resume automatically after reboot. | Must |
| FR-6 | Respect an exclusion list of apps and domains the user does not want recorded. | Must |
| FR-7 | Capture extracted search-query terms from browser history where present in the URL. | Should |
| FR-8 | Detect available Chrome profiles and ingest data only from the one or more profiles the user authorized. | Must |
| FR-9 | Support more than one browser. | Could |

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

Raw activity events should be retained only as long as needed to generate and refresh the profile; a rolling window is appropriate. The user's single-action delete must remove both raw events and the generated profile.

Dashboard history must be backed by the local activity store and obey the same rolling retention window. Remote LLM requests must use an ephemeral minimized digest created for the request rather than a persistent remote copy of the raw event store.

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
| NFR-3 | Footprint | The local store must stay within a modest disk budget via retention limits; no unbounded growth. |
| NFR-4 | Security | Local data must be protected at rest using the operating system's available protections. |
| NFR-5 | Usability | Installation, permission granting, and first profile generation must be achievable by a non-expert in minutes. |
| NFR-6 | Transparency | Privacy-relevant state (collecting / paused, what is stored) must be visible at a glance. |
| NFR-7 | Portability | Architecture should not preclude a later second platform (the MVP may target one OS first). |

---

## 8. Assumptions, dependencies, and risks

### 8.1 Assumptions

- The target user will grant the system permissions a native collector requires, given clear value and control.
- Window titles and browser URLs provide sufficient signal to produce a profile the user finds accurate and useful.
- A profile that fits within a language model's context window is sufficient for the MVP; retrieval over a larger store is not yet needed.

### 8.2 Dependencies

- Operating-system APIs for foreground-window and app-usage information.
- Read access to the local browser history store.
- A third-party language model API (e.g. Claude or GPT) for profiling and chat.
- Local access to the history databases for the Chrome profile or profiles selected by the user.

### 8.3 Key risks

| Risk | Impact | Mitigation |
|---|---|---|
| Cold start: profile too thin in the first days to feel valuable. | High — early churn | Generate a useful profile from minimal data; set expectations during onboarding; show value quickly from browser history, which is rich immediately. |
| Incorrect or creepy inference erodes trust. | High | Express uncertainty; hold back on sensitive topics; make correction and deletion trivial. |
| Privacy perception: product reads as surveillance. | High — non-adoption | Local-first architecture, transparent onboarding, visible control, honest framing. |
| Remote profiling conflicts with user expectations of local-first behavior. | High | Explain the two egress paths before collection, minimize activity locally, never transmit the complete raw log, disclose provider retention, and let the user pause collection or delete local data. |
| The wrong Chrome profile is ingested or personal profiles are mixed unintentionally. | High | Require explicit multi-profile selection during onboarding, ingest only selected profiles, label profile sources in activity history, and allow the selection to be changed later. |
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
- Users do not uninstall over privacy discomfort after seeing what is collected.

### 9.3 Explicit anti-goal

*Feature count is not a success measure. A small system that proves behavioral context changes user behavior is a success; a feature-rich system that does not is not.*

---

## 10. Future directions (post-MVP)

These are recorded to show direction. None are part of the MVP and none should influence MVP scope decisions.

- **Additional signal sources.** Email and music-service integrations via OAuth add topical and emotional signal that behavioral data alone cannot supply.
- **Proactive assistance.** Goal-aware nudges, once the interaction-design problem of avoiding notification fatigue can be addressed responsibly.
- **Context layer for other AI tools.** Exposing the profile through a standard interface so any external AI assistant can draw on it, turning the product from an app into infrastructure.
- **Second platform and sync.** Extending beyond the first operating system and across a user's devices.

**Deliberately excluded.** Capturing in-app content from social media platforms is not a viable direction: their APIs are closed and provide no access to what the user sees or does inside them. The roadmap must not depend on it.
