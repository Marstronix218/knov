# Autonomous Work Agent

## Scope

The work agent is the post-MVP layer that turns Knov's behavioral context into
delegated work. It learns repeated workflows from local activity, infers
durable goals, lets the user turn a confirmed workflow into an editable
**Skill**, and runs a deliberately small set of bounded actions with explicit
permission, verification, an audit journal, and undo where possible.

It implements the strategy's evidence-gated roadmap through Phase 4 (execute
reversible work) and the first parts of Phase 5 (scheduled and context-triggered
runs). It does not train models, does not run multi-step goal delegation, and
does not expose Knov to other AI tools. See [Requirement status](#requirement-status).

Everything in the agent is local and deterministic. **The agent never calls a
model provider.** The only provider involvement nearby is the existing,
opt-in Prediction Experiment.

## Closed loop

```text
OBSERVE → UNDERSTAND → PREDICT → PREPARE → ACT → VERIFY → LEARN
   │           │           │         │        │       │        │
activity   state, goals  workflow  skill    bounded  per-    approvals,
normalized  workflows    next step  plan    adapters action  outcomes,
                                                     checks  proposals
```

| Layer | Module | Responsibility |
| --- | --- | --- |
| Event normalizer | `agent/normalize.rs` | Converts app, browser, and editor events into semantic steps (`app:…`, `web:<domain>`, `search`) with a category; drops titles, queries, noise apps, short focus blips, excluded apps/domains, and browser focus already covered by page-level events |
| State engine | `agent/goals.rs`, `agent/mod.rs` | Current thread, inferred goal, recent steps, open resources, in-progress workflow, and unresolved work; snapshots every 10 minutes while collecting |
| Workflow miner | `agent/workflows.rs` | Finds recurring contiguous step sequences across sessions and days |
| Automation scorer | `agent/workflows.rs` | Ranks workflows by frequency, time, stability, what Knov can safely do, risk, and past skill outcomes |
| Learned Skills | `agent/skills.rs` | Editable plans with steps, trigger, exception policy, and server-validated action targets |
| Policy | `agent/policy.rs` | Risk-aware authorization, grant precedence, unattended rules, proposals, learned preferences |
| Runtime | `agent/runtime.rs` | Plan → approve → execute → verify → journal; budgets, kill switch, fail-closed recovery, rollback |
| Adapters | `agent/actions.rs` | The only side effects: open URL, open app, write draft, run allow-listed checks |
| Store | `agent/store.rs` | SQLite persistence and metrics |

## Workflow discovery

Steps are grouped into sessions separated by 20 idle minutes. A **workflow**
is a contiguous sequence of 3–6 steps that:

- contains at least **three distinct** steps (two-step alternation such as
  video → search → video is a habit, not a process);
- occurs at least **three times** (non-overlapping) on at least **two days**;
- is finished at least **15%** of the times its first two steps occur
  (`completion rate`), filtering coincidences of a common opening.

Shorter patterns subsumed by a longer one with similar support are dropped.
Workflow IDs are a hash of the step signature, so reviews survive re-mining.
Searches inside a site (for example YouTube results) stay part of that site's
step; only general web search engines produce a `search` step.

Mining runs at most every 30 minutes when new activity exists, and on demand
through **Rescan**. On a 30-day window of ~5k events it takes roughly 0.1 s in
a release build. Unreviewed workflows that stop recurring are removed;
confirmed or dismissed ones keep the user's decision but drop expired evidence.

Evidence lists only app names, domains, and URL paths, never titles or page
contents.

### Opportunity score

```text
score = (0.45·frequency + 0.15·time + 0.20·stability + 0.20·executability)
        × (1 − risk) × outcome
```

- `frequency` = per-week rate ÷ 5; `time` = average duration ÷ 30 min
- `stability` = completion rate
- `executability` weights steps Knov can **prepare** (open) at 0.5 and
  **execute** (checks in an approved workspace) at 1.0
- `risk` is 0.9 for sensitive sites (finance, health, credentials), 0.3 for
  communication, otherwise 0.1
- `outcome` blends the linked skill's completion and undo history (closed loop)

A workflow is surfaced as an **automation opportunity** only when the score is
at least 0.40, it was seen at least four times, Knov can do at least one step,
and it involves no sensitive step. The setup-time estimate (15 s per prepared
step, 60 s per check) is labelled as a rough local estimate.

## Skills

Creating a skill confirms its workflow. Each step becomes:

| Step | Default action |
| --- | --- |
| Web page | `open_url`, resolved at plan time to the latest page on that domain **in the same thread**, otherwise the workflow's stable page or site root |
| Application | `open_application` |
| Terminal, when an approved workspace matches the thread | `run_checks` with the workspace's first preset |
| Search, sensitive site, repeated step | Left to the user |

Users can rename a skill, toggle steps, point a terminal step at an approved
workspace and preset, add a "resume brief" draft step, choose how exceptions
are handled (stop or continue), and choose a trigger:

- **Only when I run it**
- **When I start this workflow** — the user's last session matches the first
  two or more steps (2-hour cooldown)
- **On a schedule** — daily or weekly at an hour (20-hour cooldown)

All targets are produced and re-validated in Rust; the interface can only
toggle steps and choose from approved workspaces and presets.

## Actions and risk

| Action | Risk class | Interrupts you | Verification | Undo |
| --- | --- | --- | --- | --- |
| Open a web resource | Ephemeral | Yes | Credential-free HTTP(S) check; macOS open exit status | Not needed |
| Open an application | Ephemeral | Yes | Name validation; macOS open exit status | Not needed |
| Write a local draft | Draft | No | Written inside Knov's Drafts folder; SHA-256 re-read | Delete if unedited |
| Run checks in a workspace | Ephemeral | No | Ran to completion; exit code captured | Not needed |
| Commit changes | Persistent, reversible | — | Not available | — |
| Send a message or email | External communication | — | Not available | — |
| Delete, pay, change security | Destructive | — | Not available | — |

**Checks** run one of eight fixed argument lists (`cargo test`, `npm test`,
`pnpm test`, `yarn test`, `python3 -m pytest -q`, `go test ./...`,
`swift test`, `make test`) without a shell, only in folders the user approved,
only when the preset's marker file exists, with `CI=1`, a 10-minute limit
(the whole process group is stopped on timeout), and output trimmed to the
last 4,000 characters with the home path and credential-looking lines
redacted. A folder can be approved only if it is inside the home folder, is
not the home folder or `~/Library`, and contains a supported project marker.
A failing test run is reported as **Needs a look**, not as a Knov failure.

**Drafts** are written to `<app data>/drafts` (mode 0700) under a validated
file name, never overwrite an existing file, and are opened only from inside
that folder. Undo refuses to delete a draft the user has edited.

## Permissions

Confidence never grants authority. For each planned action, in order:

1. Agent paused (kill switch) → **blocked**.
2. Action unavailable or external/destructive → **blocked**.
3. Any matching **Never** grant → **blocked**.
4. The most specific matching grant (skill > workspace > everywhere) decides:
   **Automatic** → allowed; **Always ask** → ask.
5. No grant → ask (read-only actions would be allowed; none exist yet).

Unattended runs (schedule or context) never open windows or apps, even under
an automatic grant; those steps wait in **Ready for you**. Manual runs always
show the plan first, even if every step is pre-approved.

Grants are created only by the user: while approving ("allow this
automatically for … from now on"), by accepting a **permission suggestion**,
or explicitly under **Delegated work → Permissions** (optionally for 7 or 30
days). Removing a skill or workspace revokes its grants.

**Progressive autonomy.** After five approvals of the same action in the same
scope with no declines, failures, or undos, Knov suggests an automatic grant.
Declining the suggestion suppresses it until approvals double.

**Budget.** Actions running under an automatic grant are limited per hour
(default 30, configurable 1–200). Over budget, they are blocked, not queued.

## Runs and the journal

Every run is persisted **before** anything executes, so what the user approves
is exactly what runs. A run records what Knov believed (thread, goal, workflow
position, recent steps), the manual steps left to the user, and each action's
target, rationale, risk class, scope, decision, permission, status, result,
verification checks, command output, and rollback plan.

Execution happens on a single background executor. It fails closed: actions
are skipped if the agent was paused after approval or (with *stop on
exception*) an earlier step needed attention; actions left running when Knov
quit are marked failed on the next launch. Closing an untouched manual preview
cancels it without counting as a decline.

## Goals and state

A goal is a thread active on at least two of the last 14 days with at least
20 minutes or eight signals. Its title combines a verb from the dominant work
category with the thread (for example "Build Knov desktop"). Users can
confirm, rename, complete, or dismiss goals; reviews persist across
re-inference. The current goal is attached to every prediction batch and to
every run.

State snapshots (thread, goal, recent step labels, workflow position,
unresolved kinds) are stored every 10 minutes while collecting and new activity
exists. They are the Phase 0 dataset for later evaluation.

## Prediction integration

The Prediction Experiment gains a third source, `workflow`: when the user is
part-way through a known workflow, Knov predicts the next step with confidence
grounded in the workflow's completion rate. Its accuracy is reported next to
the provider and heuristic baseline, along with a confidence-calibration
table. See [Prediction Engine](prediction-engine.md).

## Metrics

**Delegated work → Insights** reports, from the local journal: task completion
rate, verified-action rate, approval acceptance, undo rate, a rough estimate of
setup minutes saved, high-risk actions (always zero; none are available),
active automatic permissions, and actions in the last seven days. It also
lists learned preferences per action and scope ("You usually approve",
"Mixed"), which shape suggestions but never grant permission.

## Privacy and retention

- No agent component sends data anywhere. Checks run local commands; their
  output stays in SQLite.
- Workflows, skills, runs, actions, grants, approved workspaces, goal reviews,
  and state snapshots live in Knov's SQLite database; drafts live in Knov's
  Drafts folder.
- Snapshots follow the 30-day activity window. The action journal is kept for
  90 days as an audit trail.
- **Delete everything** removes all of the above, including drafts.

## Requirement status

Status of the strategy's proposed requirements (AR-1–AR-17):

| ID | Requirement | Status |
| --- | --- | --- |
| AR-1 | Local semantic state | Implemented |
| AR-2 | Calibrated next-intent/step/resource prediction | Partial: workflow next-step source, goal tagging, calibration reporting; no automatic recalibration |
| AR-3 | Prediction-to-outcome records | Implemented (extends the existing evaluator) |
| AR-4 | Workflow discovery with evidence | Implemented |
| AR-5 | Automation scoring | Implemented |
| AR-6 | Inspectable, editable Skills | Implemented |
| AR-7 | Bounded execution runtime | Implemented with four adapters |
| AR-8 | Grants by action, scope, duration | Implemented for everywhere / skill / workspace; no per-application scope |
| AR-9 | Risk policy | Implemented; external, destructive, financial, credential, and persistent repository actions are unavailable |
| AR-10 | Verification | Implemented per adapter |
| AR-11 | Rollback | Implemented for drafts; other actions need none |
| AR-12 | Action journal | Implemented |
| AR-13 | Progressive autonomy proposals | Implemented |
| AR-14 | Background work | Partial: scheduled and context triggers run non-interrupting granted actions; no multi-step goal delegation |
| AR-15 | Personal policy | Partial: deterministic preference summary; no learned model |
| AR-16 | Local-first privacy | Implemented |
| AR-17 | Interoperability (MCP/API) | Not implemented |

Per-user models and adapters (strategy Phase 6) are deliberately not built;
the journal, snapshots, and prediction outcomes are the data they would need.
