# Design

## Source of truth

- Status: Active
- Last refreshed: 2026-08-06
- Primary product surfaces: macOS desktop app, Chrome companion, first-run setup, contextual assistant
- Evidence reviewed: `README.md`, `knov_prd.md`, `docs/architecture.md`, `docs/privacy-model.md`, `docs/screenshots/dashboard.jpg`, `docs/screenshots/chrome-companion-settings.jpg`, `docs/brand/knov-logo-concept.png`, `apps/desktop/src/App.tsx`, `apps/desktop/src/App.css`, `apps/desktop/src/lib/mockData.ts`, `apps/desktop/src/types.ts`, and the extension UI sources
- Observed product state: the alpha collects foreground activity and selected Chrome metadata, builds a local profile, supports profile corrections and provider-backed chat, and now presents continuity through Now, Threads, evidence rails, resumable resources, and inspectable context briefs. Attention analytics remain available as secondary evidence.
- Design inference: thread reconstruction currently derives from the dashboard's available activity/topic fields. Durable user-reviewed thread identities, merges, and corrections still require a native data model rather than frontend-only derivation.
- Assumption: the first commercial audience remains AI-heavy, multi-project knowledge workers on Mac. This should be validated with 5–8 weekly users before widening the audience.

## Product thesis

### Use case

Knov helps a knowledge worker return to a project, understand what changed, and continue with an AI assistant that already has the relevant context.

The highest-value moment is not “show me how many hours I spent in Chrome.” It is:

> “I am back. What was I doing, why did it matter, and what is the best next move?”

Primary jobs to be done:

1. **Resume a thread:** recover the documents, tools, decisions, and likely next step behind a recent work session.
2. **Brief an AI:** make relevant behavioral context available without copying a long explanation into every new chat.
3. **Understand attention:** see where effort went in terms of projects and outcomes, not only applications and websites.
4. **Inspect memory:** know what Knov believes, why it believes it, and correct or remove it.
5. **Protect boundaries:** capture less than screen-recording products while making pause, exclusion, retention, and egress unmistakable.

Secondary jobs:

- Prepare a daily or weekly reflection.
- Find a recently visited resource by project or approximate time.
- Notice abandoned or repeatedly resumed work.
- Export a concise context packet to another AI tool.

### Problem statement

AI assistants remember what users explicitly tell them inside their own chat histories, while activity trackers know what applications were used but not why that work mattered. Screen-memory products can reconstruct more, but they do so by capturing far more sensitive content than many people are willing to record.

As a result, multi-project knowledge workers repeatedly reconstruct the same context: what they were doing, which resources mattered, what they decided, and what they intended to do next. This creates task-switching cost, generic AI responses, and abandoned threads.

Knov should solve this with a user-owned context layer that derives useful continuity from the minimum necessary behavioral signal, keeps detailed history local, shows its evidence, and can serve more than one AI assistant.

### Positioning

**Category:** private work-context layer

**One-line promise:** Pick up where you left off—without recording everything.

**Expanded promise:** Knov quietly turns app and browser activity into reviewable project context, then helps you resume work or brief the AI you already use.

**Positioning contrast:**

- More useful than a time tracker because it reconstructs intent and continuity.
- Less invasive than a screen or audio recorder because it deliberately does not capture pixels, page bodies, keystrokes, clipboard contents, or audio.
- More portable than built-in assistant memory because the user owns and corrects the context independently of one model vendor.
- More automatic than a notes app because the first draft of memory comes from behavior rather than manual capture.

## Competitive landscape

The practical market is four overlapping categories. “All competitors” should be maintained as a living set; the products below are the strongest current reference points rather than an exhaustive directory.

| Category / product | What it does well | Gap Knov can own | Product implication |
| --- | --- | --- | --- |
| [Pieces](https://pieces.app/) | On-device long-term work memory, chronological activity, conversational search, integrations, and an MCP surface | Broader and more mature memory platform; Knov can be simpler, more legible, and explicitly metadata-minimal | Make continuity and evidence clearer; add a vendor-neutral context export/API before adding more capture |
| [screenpipe](https://screenpipe.com/about) | Local-first screen, app, browser, and audio memory; natural-language search; plugins and MCP | Captures much richer and more sensitive content with higher storage/compute cost | Turn “no screenshots, no audio, no page bodies” into a visible product advantage, not a footnote |
| Microsoft [Recall](https://support.microsoft.com/en-us/windows/privacy/privacy-and-control-over-your-recall-experience) | OS-level visual recall, local processing, natural-language retrieval, snapshot timeline, filters | Windows/Copilot+ specific and screenshot-based; optimized for finding rather than model-neutral context | Do not imitate a visual DVR; focus on Mac, project continuity, and portable context |
| [ActivityWatch](https://activitywatch.net/) | Free, open-source, local activity and browser tracking with strong privacy credibility | Primarily reports behavior; limited opinionated continuity and AI assistance | Basic app/site charts are commodity. Build the experience around threads, decisions, and next steps |
| [Timing](https://timingapp.com/features) | Polished native Mac tracking, projects, categorization, AI summaries, billing, reporting, integrations | Optimized for accounting for time rather than remembering and continuing work | Do not compete on timesheets, billing, or productivity scores |
| ChatGPT [Memory](https://help.openai.com/en/articles/8590148-memory-daq) | Automatically synthesized, editable memory from chats, files, and connected apps | Memory is rooted in what reaches ChatGPT and tied to its ecosystem | Let users generate a trusted context packet for any assistant and show behavioral evidence behind it |
| Claude [Memory](https://support.claude.com/en/articles/11817273-use-claude-s-chat-search-and-memory-to-build-on-previous-context) | Categorized memory, project separation, past-chat search, citations, and user editing | Memory remains conversation-derived and Claude-specific | Project boundaries, citations, and editable memory are table stakes; behavior-derived continuity is the wedge |
| Gemini [Personal Intelligence](https://support.google.com/gemini/answer/16836988?hl=en) | Personalization across Google services, past chats, and connected apps | Powerful but cloud/account-ecosystem dependent and less user-owned | Avoid a breadth race; win on local control, transparency, and cross-provider portability |
| [Mem](https://mem.ai/) / [Recall](https://www.recall.it/about) | Fast capture, AI-organized notes or saved content, retrieval, synthesis, and resurfacing | Depends on content the user deliberately saves or creates | Let users promote a behavioral thread into a durable note, but do not become another notes editor |

Historical note: Rewind/Limitless defined the “perfect memory” category, but the product’s move toward wearable/audio capture and later acquisition make it a strategic lesson more than the main product template. Knov should avoid promising omniscience; trustworthy selectivity is more defensible.

### Competitive feature posture

| Capability | Knov priority | Reason |
| --- | --- | --- |
| App/window/browser metadata capture | Keep and polish | Foundational signal with relatively low invasiveness |
| Screenshots, OCR, audio, page bodies | Explicit non-goal | Destroys the clearest trust and resource-use advantage |
| Project/thread reconstruction | Highest | Converts commodity telemetry into user value |
| Natural-language recall | High | Expected in the category and useful once evidence retrieval is strong |
| Cross-provider context handoff | Highest | Creates a defensible role outside any single assistant |
| Profile/memory inspection and correction | Highest | Essential for trust and better than opaque personalization |
| Time tracking reports and billing | Low / non-goal | Mature competitors already own this use case |
| Autonomous actions | Later | Requires stronger trust, intent, and permission architecture |

## Brand

- Personality: perceptive, calm, private, precise, quietly optimistic
- Trust signals: concrete data labels, source and timestamp citations, local/remote boundary indicators, visible capture state, plain-language exclusions, reversible controls
- Avoid: surveillance language, quantified-self judgment, “knows everything,” productivity guilt, cyberpunk dashboards, generic AI sparkles, anthropomorphic certainty
- Brand idea: **a field guide to your working context**, not an omniscient observer
- Naming rule: use **Knov** consistently. Existing screenshots and extension copy that show “Knoveyla” should be treated as stale brand artifacts unless a rename is explicitly chosen.

## Product goals

- Goals:
  - Make the user meaningfully ready to resume a recent project within 60 seconds of opening Knov.
  - Make context reuse across AI tools feel easier than re-explaining the work.
  - Make every inferred claim inspectable and correctable.
  - Make the product’s deliberately limited capture model obvious and desirable.
  - Create a repeated daily loop rather than a dashboard users admire once and stop opening.
- Non-goals:
  - Employee monitoring, scoring, billing, or managerial reporting.
  - Comprehensive screen/audio recall.
  - Replacing the user’s primary chat assistant, notes app, task manager, or browser.
  - Diagnosing focus, health, or productivity.
- Success signals:
  - Weekly active use after the novelty week.
  - At least two successful “resume thread” sessions per retained user per week.
  - Median time from launch to opening the relevant resource or sending a context-aware prompt under 60 seconds.
  - Users rate a majority of surfaced thread summaries as accurate enough to act on.
  - Context packets are reused in external assistants or the built-in assistant.
  - Low privacy-related disable/uninstall rate after onboarding.
  - Corrections reduce repeated bad inferences over time.

## Personas and jobs

- Primary persona: an AI-heavy developer, researcher, founder, designer, or freelancer on Mac who juggles 3–8 active work threads and frequently changes tools.
- Secondary persona: a privacy-aware professional who wants useful personal analytics but refuses continuous screen/audio recording.
- User jobs:
  - “Help me continue the thing I was doing yesterday.”
  - “Give Claude/ChatGPT/Cursor only the context it needs for this task.”
  - “Show me what I actually moved forward this week.”
  - “Show me why you think this is one of my projects.”
  - “Let me remove a sensitive or incorrect memory permanently.”
- Key contexts of use: start of day, return after a meeting or interruption, switching between projects, starting a new AI chat, end-of-day reflection, weekly review

## User workflow

### First-run workflow

1. **Understand the boundary:** lead with what Knov intentionally does not capture, then explain what it does capture and what can leave the Mac.
2. **Choose sources:** approve apps/browser profiles and offer sensible default exclusions for password managers, financial, health, and private browsing contexts.
3. **Choose intelligence mode:** built-in provider via BYOK for alpha; later offer local-only summarization and “capture now, connect AI later.”
4. **Build first context:** show progressive status and useful raw facts while the first profile is generated instead of blocking on a blank setup screen.
5. **First reveal:** present 3–5 detected work threads with evidence and ask the user to confirm, rename, merge, or discard them.
6. **Aha action:** let the user choose one thread and immediately generate a “resume brief” or context packet.

### Daily workflow

1. Open Knov to **Now**.
2. See one primary card: “Continue where you left off,” including project, last active time, resources, a concise state summary, and suggested next move.
3. Choose **Resume**, **Ask with this context**, **Copy context**, or **Not this**.
4. Resume opens the relevant resource(s) and keeps a small evidence drawer available.
5. Knov learns from explicit corrections and later behavior, without treating silence as confirmation.

### Weekly workflow

1. Review “What moved” by project/thread, not by app.
2. Inspect completed, advancing, stalled, and newly discovered threads.
3. Confirm or correct durable memory.
4. Export a short weekly brief if desired.

### Failure and trust workflow

1. Every inferred statement has a **Why this?** affordance.
2. Evidence shows source type, timestamp, and exact included metadata.
3. The user can correct, exclude source, forget item, or lower its scope to one project.
4. Corrections are visibly durable and never silently overwritten by regeneration.

## Information architecture

- Primary navigation:
  - **Now** — the daily continuity surface and universal ask box
  - **Threads** — active projects/work streams reconstructed from behavior
  - **Memory** — durable profile, corrections, and provenance
  - **Activity** — raw local timeline and search for auditability
  - **Settings** — capture, privacy, provider, extension, retention, deletion
- Core routes/screens:
  - Replace current **Overview** with **Now**.
  - Fold the standalone **Assistant** into Now and each Thread. Chat is an action on context, not the product’s separate destination.
  - Rename **Profile** to **Memory** and organize it by projects, preferences, working style, and user-authored truths.
  - Keep Activity as the factual ledger rather than the home experience.
- Content hierarchy on Now:
  1. Continue where you left off
  2. Ask using current context
  3. Active threads
  4. Since you were away / changes worth knowing
  5. Attention summary as supporting evidence

## Design principles

1. **Continuity before analytics.** Lead with what the user can continue, not telemetry about the past.
2. **Minimum necessary signal.** Product value must not depend on screen, audio, keystroke, or page-body capture.
3. **Evidence before confidence theater.** Show why an inference exists; avoid unexplained percentages and certainty labels.
4. **User truth outranks model inference.** Corrections persist and remain visually distinct.
5. **One useful next move.** Prefer one well-supported action over a feed of generic recommendations.
6. **Context is portable.** The user’s memory should help across assistants and tools, not trap them in Knov chat.
- Tradeoffs:
  - Less captured content means some recall questions cannot be answered; Knov should state that plainly.
  - Strong privacy controls add setup complexity; staged consent and progressive value should reduce the burden.
  - Project inference will sometimes be wrong; low-friction merge/rename/correct controls are core product interactions, not settings cleanup.

## Visual language

- Color: retain near-black and electric chartreuse as recognizable brand assets, but use the accent for state and action rather than coating every insight. Add warm off-white for reading surfaces and restrained ice blue for observed evidence. Reserve amber/red for uncertainty and risk.
- Typography: use a strong editorial display face only for large moments if licensing and bundle size permit; otherwise use the native system family with more generous body sizes. Minimum routine body text should be 13–14 px in the desktop UI; current 8–11 px metadata is too small for a product built on trust and close reading.
- Spacing/layout rhythm: 8 px base, 16–24 px card spacing, 32–48 px section spacing. Reduce the number of simultaneously bordered boxes.
- Shape/radius/elevation: 12–16 px surfaces, hairline borders, shallow layered elevation, one luminous focal surface per view.
- Motion: use slow, purposeful transitions for thread formation, evidence expansion, and resume state. Avoid ambient pulsing except for the capture-status indicator. Respect reduced motion.
- Imagery/iconography: use the existing K mark and simple line icons. Introduce a restrained “context constellation” motif—resources and sessions connected around a thread—but never obscure chronology or evidence.
- Data visualization: default to project threads, temporal bands, and evidence trails. App/site charts move to a secondary Attention view.

### Signature Now composition

- A quiet top line: “Thursday, August 6 · 4 active threads · collection on.”
- A large focal card: **Continue: Knov onboarding** with a 2–3 sentence state summary.
- An evidence rail below it: VS Code → Tauri docs → extension settings, with times and source labels.
- Primary actions: **Resume thread**, **Ask with context**, **Copy brief**.
- A compact command field: “Ask about your work…” with scope set to Current thread / Today / All memory.
- A visual thread field underneath, where recency, momentum, and confidence are visible without turning work into a score.

## Components

- Existing components to reuse: capture status, range selector, activity rows, recommendation evidence disclosure, provider settings, exclusions, profile corrections, modal primitives
- New/changed components:
  - ResumeCard
  - ThreadCard and ThreadMap
  - EvidenceRail and EvidenceDrawer
  - ContextScopePicker
  - ContextPacketPreview
  - MemoryClaim with observed/inferred/user-authored states
  - FirstRevealReview for confirm/rename/merge/discard
  - PrivacyBoundary strip showing Local facts → Minimized digest → Selected provider
- Variants and states:
  - Thread: active, cooling, stalled, completed, unknown
  - Claim: observed, inferred, user-authored, corrected, disputed
  - Context packet: built-in assistant, clipboard, MCP/API destination (future)
  - Evidence: locally available, excluded, expired, unavailable
- Token/component ownership: extend `apps/desktop/src/App.css` variables initially; do not add a second design-system dependency during alpha validation.

## Accessibility

- Target standard: WCAG 2.2 AA for the desktop webview and extension surfaces
- Keyboard/focus behavior: all navigation, thread actions, evidence drawers, filters, setup steps, and correction controls must work without a pointer; provide visible `:focus-visible` rings.
- Contrast/readability: increase metadata size and contrast; do not encode source, confidence, or collection state through color alone.
- Screen-reader semantics: use landmarks, real headings, labeled graphs with text summaries, table semantics for detailed activity, and explicit expanded/collapsed state.
- Reduced motion and sensory considerations: support `prefers-reduced-motion`; no mandatory animated graph; no persistent decorative flicker.

## Responsive behavior

- Supported breakpoints/devices: desktop-first from 900 px upward; functional compact view at 720–899 px for smaller windows; extension popup/options remain separate responsive surfaces.
- Layout adaptations: sidebar collapses to icons or a top rail; the focal ResumeCard remains first; evidence rails become horizontally scrollable or vertical; analytics stack below continuity content.
- Touch/hover differences: never hide required explanation or controls behind hover; hover may preview evidence, while click/keyboard opens it.

## Interaction states

- Loading: show which local sources are being aggregated and allow Activity/Settings access while synthesis runs.
- Empty: offer a useful path—start collection, select a thread manually, or ask about existing raw activity—rather than “nothing here.”
- Error: distinguish local collection, extension connection, provider, and inference failures with recovery actions.
- Success: show the generated context packet or opened resource and preserve an undoable recent action where appropriate.
- Disabled: explain why an action is unavailable and whether raw local data is still present.
- Offline/slow network: raw timeline, thread browsing, corrections, and existing memories remain usable; mark provider-backed synthesis as waiting rather than failed.

## Content voice

- Tone: calm, specific, provisional when inferred, never judgmental
- Terminology:
  - Use **thread** for a reconstructed stream of work.
  - Use **memory** for durable, reviewable context.
  - Use **activity** for raw observed events.
  - Use **context brief** or **context packet** for portable AI-ready summaries.
  - Avoid “productivity score,” “distraction,” “wasted time,” and “Knov knows.”
- Microcopy rules:
  - State evidence and uncertainty directly: “Likely related because…”
  - Pair every capture statement with its boundary: “Window titles on; screenshots never captured.”
  - Prefer action language: “Resume this thread” over “Recommendation.”

## Implementation constraints

- Framework/styling system: Tauri 2, React, TypeScript, Rust, SQLite, plain CSS variables, Lucide icons
- Design-token constraints: evolve the current variables and component classes before introducing a token framework.
- Performance constraints: the daily Now view should render from local cached thread/profile data immediately; provider refresh should be asynchronous and never block navigation.
- Compatibility constraints: Apple Silicon macOS 26 alpha, Chrome companion, Accessibility permission optional for richer window titles
- Privacy constraints: preserve current local raw storage, explicit provider egress, 30-day detailed retention, exclusions, pause, and deletion boundaries.
- Test/screenshot expectations: add component tests for new workflow states, keyboard/focus checks, responsive screenshots at primary widths, and regression coverage for durable corrections and provider-offline behavior.

## Recommended product sequence

### Phase 1 — Prove continuity (highest priority)

1. [Implemented] Replace Overview’s first screen with a ResumeCard generated from existing activity/profile data.
2. [Partial] Add explicit thread grouping and evidence review. Durable rename, merge, confirm, and discard require the native thread model.
3. [Implemented] Generate an inspectable context packet with Copy and “Ask with this context.”
4. [Next] Measure resume actions, context reuse, correction rate, and next-day/next-week return.

### Phase 2 — Become model-neutral

1. Add a local read-only context API or MCP server with explicit scopes and per-request approval/logging.
2. Provide presets for current thread, today, last seven days, and selected memories.
3. Preserve provider-specific built-in chat as a convenient client, not the only destination.

### Phase 3 — Strengthen recall without broader surveillance

1. Add natural-language search over locally retained metadata and derived threads.
2. Improve resource reopening and link/file resolution.
3. Add optional calendar/task metadata only when it materially improves thread boundaries and can follow the same consent model.

### Defer

- Screenshot/OCR/audio capture
- Team monitoring
- Billing/timesheets
- Broad autonomous actions
- More dashboard charts

## Open questions

- [ ] Validate whether “resume work” or “brief my AI” produces the stronger repeated habit / product owner / determines Now’s primary CTA.
- [ ] Test whether users understand and value metadata-minimal capture without assuming Knov can search page contents / product owner / affects promise and onboarding.
- [ ] Decide whether Knov remains the product name; remove remaining “Knoveyla” artifacts after confirmation / brand owner / affects all surfaces.
- [ ] Define the first thread-clustering quality threshold and correction UX / engineering + product / affects trust.
- [ ] Decide whether local-model inference is required for the first non-technical release / product + privacy / affects onboarding and operating cost.
- [ ] Define MCP/API permission scopes and audit log before implementation / security + product / affects model-neutral positioning.
- [ ] Recruit 5–8 target alpha users for a two-week diary study focused on resume moments and re-explanation / product / validates the central hypothesis.
