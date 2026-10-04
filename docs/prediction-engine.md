# Prediction Engine

## Scope

The Prediction Engine is an opt-in technical-alpha experiment that tests
whether Knov can predict the user's likely next work intent, useful action, or
resource better than its existing thread-continuity heuristic. It does not
train a model and does not execute work autonomously.

The MVP stops at:

```text
Observe -> Understand -> Predict -> Prepare / Resume
```

## Data flow

```text
recent non-excluded activity + semantic threads
                       |
                       v
          normalized current-work state
                       |
          +------------+-------------+
          |                          |
          v                          v
local historical retrieval    heuristic baseline
          |
          v
sanitize and minimize
          |
          v
configured BYOK provider -> up to three candidates
          |                          |
          +-------------+------------+
                        v
                 local SQLite
                        |
      later local activity + optional feedback
                        |
                        v
             deterministic evaluation
```

React uses typed Tauri commands to read settings, visible predictions, history,
and metrics or to submit feedback. Rust owns activity access, exclusions,
historical retrieval, sanitization, provider calls, persistence, and evaluation.
The frontend does not access SQLite or provider credentials.

## Current-work state and history

Rust summarizes the previous 60 minutes of useful activity into a
normalized state. Depending on available evidence, it can include the active
application, a sanitized window or page label, domain, semantic thread,
recent applications and domains, session duration, and local time features.

State construction excludes configured applications and domains. It does not
expose absolute paths, credentials, or raw document/page content. Historical
retrieval remains local and favors inexpensive signals already available in
Knov, including the same thread, application, and domain. Each retrieved
example summarizes a short activity sequence so transition patterns remain
visible without sending full events. The provider receives only a small
sanitized representation of relevant patterns.

## Candidates and baseline

Each candidate records an intent, next action, optional supported resource and
thread, confidence from 0 to 1, prediction horizon, short reasoning summary,
and visible evidence. The provider can return up to three ranked candidates.
The default horizon is 20 minutes; provider-supplied horizons are constrained
to 5–120 minutes.
The prompt asks it to predict meaningful work intent rather than superficial UI
clicks, lower confidence when evidence is weak, and avoid unsupported resources.
Malformed structured output cannot create provider candidates; the local
baseline remains available and other Knov features continue unaffected.

Every batch includes a deterministic heuristic candidate based on existing
thread continuity and recent resource/app evidence. When the user is part-way
through a workflow the [work agent](autonomous-agent.md) has learned, the batch
also includes a `workflow` candidate: the workflow's next step, with confidence
grounded in how often that workflow is finished once started and how far along
the user is. All sources use the same persistence and evaluation path so their
top-ranked accuracy can be compared.

Each candidate also records the user's inferred goal at prediction time
(`predicted_goal`) and, for workflow candidates, the workflow ID, giving the
goal → workflow → next step → resource hierarchy a local record.

## Triggering and display

**Prediction experiment** is off by default. When enabled, every generation
requires:

- active collection;
- enough meaningful, non-excluded recent context;
- no prediction generation already in flight.

Scheduled generation additionally requires useful activity since the preceding
batch and the configured cooldown, initially 15 minutes, to have elapsed. A
provider key is optional: without one, batches contain only the local baseline
and workflow candidates.

The scheduler accepts cooldown values from 10–120 minutes. Manual generation
uses the same in-flight guard and state/persistence path.

The initial display threshold is 0.65. Unexpired provider and workflow
candidates at or above the threshold can appear under **Likely next** on Now. Lower-confidence results
remain in Shadow Mode for local evaluation. Visible cards use short evidence,
not chain-of-thought, and allow optional correct, wrong, or dismiss feedback.
Candidates marked wrong or dismissed are excluded from **Likely next** on every
subsequent load, not only for the current session.

**Resume predicted work** delegates only to Knov's existing safe thread/resource
resumption. The Prediction Engine does not modify files, run shell commands,
send messages, submit forms, make purchases, delete data, or click through the
operating system.

## Local persistence

The `predictions` table stores one row per candidate. Its concepts are:

| Group | Stored fields |
| --- | --- |
| Identity | prediction ID, batch ID, rank, creation time |
| Source | `heuristic`, `workflow`, or `provider` |
| Hierarchy | inferred goal at prediction time; workflow ID for workflow candidates |
| Candidate | intent, action, optional resource type/label/safe locator, optional thread ID |
| Scoring | confidence, horizon minutes, reasoning summary, evidence |
| Context | sanitized state summary, not the raw provider prompt |
| Evaluation | `pending`, `matched`, `partial`, `missed`, or `expired`; observed outcome; match score; evaluation time |
| Feedback | optional user feedback and reason |

Prediction settings live with the existing local settings record. **Delete
everything** removes prediction rows, evaluation results, feedback, and settings
alongside other app-owned data.

## Deterministic evaluation

After the horizon passes, Rust compares a prediction with later local activity.
Signals can include whether the predicted thread became active, the predicted
application/domain/resource appeared, and whether the predicted action overlaps
with the resulting semantic thread. Explicit correct/incorrect feedback
overrides the deterministic score to 1 or 0 respectively; dismiss remains
feedback without asserting correctness.

The evaluator emits a match score from 0 to 1. MVP thresholds are configurable
constants, not scientifically validated boundaries:

- 0.75 or greater: `matched`
- 0.40 through 0.74: `partial`
- below 0.40: `missed`
- insufficient observation after expiry: `expired`

Prediction History exposes candidate time, text, confidence, source, outcome,
match score, and feedback. Aggregate local metrics include evaluated counts,
matched/partial/missed totals, provider, baseline, and workflow top-1 accuracy,
high-confidence accuracy for candidates at 0.75 or above, positive-feedback
rate, and a calibration table comparing mean stated confidence with the
observed match rate in four confidence bands.

## Privacy and failure isolation

Raw activity and historical retrieval remain local. A prediction request can
send only minimized, sanitized current-work features and at most five relevant
historical sequences directly to the configured provider. It excludes complete
history, full URLs, absolute local paths, credentials, excluded activity, and
unrelated events. URLs are reduced to domains; any returned safe locator must
be credential-free HTTP(S), and Knov removes its query and fragment.
Provider-side handling remains governed by the user's provider account.

Prediction is non-critical. A missing key, unavailable provider, malformed
response, timeout, insufficient context, or evaluation error must not stop
collection, the existing Now fallback, profile generation, or chat. Schema
changes use the existing transactional migration path. New prediction
generation also stops when the experiment is disabled or collection is paused.

## Future dataset path

Local records provide the core of a future, separately approved supervised
dataset:

```text
state at prediction time
+ relevant historical context
+ predicted action
+ actual future behavior
+ user feedback
+ evaluation score
```

This MVP only collects and evaluates those records. It does not fine-tune,
train, or run a dedicated prediction model, and it does not add a cloud Knov
service or ML backend.

The MVP persists the sanitized current-state summary, prediction, compact
observed-outcome summary, score, and feedback. It intentionally does not store
the raw provider prompt or a separate historical-sequence snapshot. Exact
long-term reconstruction of historical context or raw future activity would
therefore require a later schema/retention decision; it is not claimed by this
MVP.
