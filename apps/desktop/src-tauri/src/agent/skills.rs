//! Learned Skills: an editable, inspectable plan derived from a confirmed
//! workflow. Users can toggle steps, retarget terminal steps to approved
//! checks, choose a trigger, and choose how exceptions are handled. Action
//! targets always originate on the Rust side and are re-validated.

use std::collections::HashSet;

use chrono::{Local, TimeZone};
use uuid::Uuid;

use super::{
    actions::{check_preset, ActionSpec},
    models::{
        ApprovedWorkspace, Skill, SkillStats, SkillStep, SkillTrigger, SkillUpdate, WorkState,
        Workflow,
    },
    workflows::step_capability,
};
use crate::{
    error::{AppError, AppResult},
    platform::{normalized_application_name, reopenable_web_url},
    prediction::sanitize_text,
};

pub(crate) const BRIEF_STEP_ID: &str = "brief";
const MAX_NAME_CHARS: usize = 80;

pub(crate) fn skill_from_workflow(
    workflow: &Workflow,
    workspaces: &[ApprovedWorkspace],
    now: i64,
) -> Skill {
    let workspace = workspace_for_thread(workflow.stats.thread.as_deref(), workspaces);
    let mut seen = HashSet::new();
    let steps = workflow
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let first_time = seen.insert(step.key.clone());
            let capability = step_capability(step, workspace.is_some());
            let action = if !first_time {
                None
            } else {
                match capability {
                    "execute" => workspace.and_then(|workspace| {
                        workspace
                            .check_presets
                            .first()
                            .map(|preset| ActionSpec::RunChecks {
                                workspace_id: workspace.id.clone(),
                                preset: preset.id.clone(),
                            })
                    }),
                    "prepare" if step.kind == "app" => Some(ActionSpec::OpenApplication {
                        app: step.label.clone(),
                    }),
                    "prepare" => step.resource.clone().map(|url| ActionSpec::OpenUrl {
                        url,
                        resolve_domain: step
                            .key
                            .strip_prefix("web:")
                            .map(|domain| domain.split('/').next().unwrap_or(domain).to_string()),
                    }),
                    _ => None,
                }
            };
            let title = match (&action, first_time) {
                (Some(ActionSpec::RunChecks { preset, .. }), _) => format!(
                    "Run `{}` in {}",
                    check_preset(preset).map_or(preset.as_str(), |preset| preset.label),
                    workspace.map_or("the workspace", |workspace| workspace.label.as_str())
                ),
                (_, false) => format!("Return to {}", step.label),
                _ => step.title.clone(),
            };
            SkillStep {
                id: format!("step-{}", index + 1),
                title,
                category: step.category.clone(),
                action,
                enabled: true,
            }
        })
        .collect::<Vec<_>>();
    let trigger_hint = match (&workflow.stats.typical_weekday, workflow.stats.typical_hour) {
        (Some(day), Some(hour)) => format!(" It usually happens on {day}s around {hour}:00."),
        (Some(day), None) => format!(" It usually happens on {day}s."),
        _ => String::new(),
    };
    Skill {
        id: format!("skill-{}", Uuid::new_v4()),
        name: sanitize_text(&workflow.title, MAX_NAME_CHARS),
        description: format!(
            "Learned from {} observed runs of this workflow.{trigger_hint}",
            workflow.stats.occurrences
        ),
        workflow_id: Some(workflow.id.clone()),
        thread: workflow.stats.thread.clone(),
        trigger: SkillTrigger {
            kind: "manual".into(),
            weekday: None,
            hour: None,
        },
        on_exception: "stop".into(),
        steps,
        enabled: true,
        created_at: now,
        updated_at: now,
        last_triggered_at: None,
        stats: SkillStats::default(),
    }
}

/// Picks the approved workspace whose folder name matches the workflow's
/// thread, if any. Checks never run in an unrelated workspace by default.
pub(crate) fn workspace_for_thread<'a>(
    thread: Option<&str>,
    workspaces: &'a [ApprovedWorkspace],
) -> Option<&'a ApprovedWorkspace> {
    let thread = thread?.to_ascii_lowercase();
    workspaces.iter().find(|workspace| {
        let label = workspace.label.to_ascii_lowercase();
        !workspace.check_presets.is_empty() && (thread.contains(&label) || label.contains(&thread))
    })
}

pub(crate) fn apply_update(
    existing: &Skill,
    update: SkillUpdate,
    workspaces: &[ApprovedWorkspace],
    now: i64,
) -> AppResult<Skill> {
    let name = sanitize_text(update.name.trim(), MAX_NAME_CHARS);
    if name.is_empty() {
        return Err(AppError::InvalidInput("Give the skill a name.".into()));
    }
    let trigger = validate_trigger(&update.trigger, existing.workflow_id.is_some())?;
    if !matches!(update.on_exception.as_str(), "stop" | "continue") {
        return Err(AppError::InvalidInput(
            "Exception handling must be stop or continue.".into(),
        ));
    }
    let mut steps = existing
        .steps
        .iter()
        .filter(|step| step.id != BRIEF_STEP_ID)
        .cloned()
        .collect::<Vec<_>>();
    for step_update in &update.steps {
        let Some(step) = steps.iter_mut().find(|step| step.id == step_update.id) else {
            if step_update.id == BRIEF_STEP_ID {
                continue;
            }
            return Err(AppError::InvalidInput(
                "A skill step was not recognized.".into(),
            ));
        };
        step.enabled = step_update.enabled;
        if step.category == "terminal" {
            step.action = match &step_update.workspace_id {
                Some(workspace_id) => {
                    let workspace = workspaces
                        .iter()
                        .find(|workspace| &workspace.id == workspace_id)
                        .ok_or_else(|| {
                            AppError::InvalidInput("Approve that workspace first.".into())
                        })?;
                    let preset = step_update
                        .check_preset
                        .clone()
                        .or_else(|| {
                            workspace
                                .check_presets
                                .first()
                                .map(|preset| preset.id.clone())
                        })
                        .ok_or_else(|| {
                            AppError::InvalidInput("That workspace has no supported checks.".into())
                        })?;
                    if !workspace
                        .check_presets
                        .iter()
                        .any(|candidate| candidate.id == preset)
                    {
                        return Err(AppError::InvalidInput(
                            "That check is not available in the workspace.".into(),
                        ));
                    }
                    step.title = format!(
                        "Run `{}` in {}",
                        check_preset(&preset).map_or(preset.as_str(), |value| value.label),
                        workspace.label
                    );
                    Some(ActionSpec::RunChecks {
                        workspace_id: workspace.id.clone(),
                        preset,
                    })
                }
                None => {
                    let app = terminal_app_label(step);
                    step.title = format!("Run commands in {app}");
                    Some(ActionSpec::OpenApplication { app })
                }
            };
        }
    }
    for step in &steps {
        if let Some(action) = &step.action {
            validate_spec(action, workspaces)?;
        }
    }
    if update.include_brief {
        steps.push(brief_step());
    }
    Ok(Skill {
        name,
        description: update
            .description
            .map(|value| sanitize_text(value.trim(), 280))
            .unwrap_or_else(|| existing.description.clone()),
        trigger,
        on_exception: update.on_exception,
        enabled: update.enabled,
        steps,
        updated_at: now,
        ..existing.clone()
    })
}

fn terminal_app_label(step: &SkillStep) -> String {
    match &step.action {
        Some(ActionSpec::OpenApplication { app }) => app.clone(),
        _ => step
            .title
            .rsplit(" in ")
            .next()
            .filter(|value| !value.contains('`'))
            .unwrap_or("Terminal")
            .to_string(),
    }
}

pub(crate) fn brief_step() -> SkillStep {
    SkillStep {
        id: BRIEF_STEP_ID.into(),
        title: "Save a resume brief to Drafts".into(),
        category: "notes".into(),
        action: Some(ActionSpec::WriteDraft {
            template: "thread_brief".into(),
        }),
        enabled: true,
    }
}

fn validate_trigger(trigger: &SkillTrigger, has_workflow: bool) -> AppResult<SkillTrigger> {
    match trigger.kind.as_str() {
        "manual" => Ok(SkillTrigger {
            kind: "manual".into(),
            weekday: None,
            hour: None,
        }),
        "context" if has_workflow => Ok(SkillTrigger {
            kind: "context".into(),
            weekday: None,
            hour: None,
        }),
        "context" => Err(AppError::InvalidInput(
            "Only skills learned from a workflow can start when you begin that workflow.".into(),
        )),
        "schedule" => {
            let hour = trigger
                .hour
                .filter(|hour| *hour < 24)
                .ok_or_else(|| AppError::InvalidInput("Choose an hour between 0 and 23.".into()))?;
            if trigger.weekday.is_some_and(|day| day > 6) {
                return Err(AppError::InvalidInput("Choose a valid weekday.".into()));
            }
            Ok(SkillTrigger {
                kind: "schedule".into(),
                weekday: trigger.weekday,
                hour: Some(hour),
            })
        }
        _ => Err(AppError::InvalidInput("Unsupported trigger.".into())),
    }
}

pub(crate) fn validate_spec(spec: &ActionSpec, workspaces: &[ApprovedWorkspace]) -> AppResult<()> {
    match spec {
        ActionSpec::OpenUrl { url, .. } => reopenable_web_url(url).map(|_| ()),
        ActionSpec::OpenApplication { app } => normalized_application_name(app).map(|_| ()),
        ActionSpec::WriteDraft { template } => {
            matches!(template.as_str(), "thread_brief" | "workflow_checklist")
                .then_some(())
                .ok_or_else(|| AppError::InvalidInput("Unknown draft template.".into()))
        }
        ActionSpec::RunChecks {
            workspace_id,
            preset,
        } => {
            let workspace = workspaces
                .iter()
                .find(|workspace| &workspace.id == workspace_id)
                .ok_or_else(|| AppError::InvalidInput("The workspace is not approved.".into()))?;
            workspace
                .check_presets
                .iter()
                .any(|candidate| &candidate.id == preset)
                .then_some(())
                .ok_or_else(|| AppError::InvalidInput("That check is not available.".into()))
        }
    }
}

/// Markdown for draft actions. Built only from the sanitized local state and
/// the skill definition; nothing is sent anywhere.
pub(crate) fn render_draft(
    template: &str,
    skill: &Skill,
    state: &WorkState,
    now: i64,
) -> (String, String) {
    let date = Local
        .timestamp_opt(now, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();
    let subject = state
        .active_thread
        .clone()
        .or_else(|| skill.thread.clone())
        .unwrap_or_else(|| skill.name.clone());
    let mut lines = Vec::new();
    let title = if template == "workflow_checklist" {
        format!("Checklist: {}", skill.name)
    } else {
        format!("Resume brief: {subject}")
    };
    lines.push(format!("# {title}"));
    lines.push(String::new());
    lines.push(format!(
        "_Prepared locally by Knov on {date}. Review before relying on it._"
    ));
    lines.push(String::new());
    if let Some(goal) = &state.goal {
        lines.push(format!("**Goal:** {} ({})", goal.title, goal.status));
        lines.push(String::new());
    }
    if template == "workflow_checklist" {
        lines.push("## Steps".into());
        for step in skill
            .steps
            .iter()
            .filter(|step| step.enabled && step.id != BRIEF_STEP_ID)
        {
            lines.push(format!("- [ ] {}", step.title));
        }
    } else {
        lines.push("## Where you left off".into());
        if state.recent_steps.is_empty() {
            lines.push("- No recent activity was observed.".into());
        }
        for step in state.recent_steps.iter().rev().take(6) {
            lines.push(format!("- {}", step.title));
        }
        if !state.open_resources.is_empty() {
            lines.push(String::new());
            lines.push("## Key resources".into());
            for resource in state.open_resources.iter().take(6) {
                match &resource.locator {
                    Some(locator) => lines.push(format!("- [{}]({locator})", resource.label)),
                    None => lines.push(format!("- {}", resource.label)),
                }
            }
        }
        if let Some(progress) = &state.workflow_progress {
            lines.push(String::new());
            lines.push("## Likely next step".into());
            lines.push(format!(
                "- {} (step {} of {} in “{}”)",
                progress.next_step.title,
                progress.matched_steps + 1,
                progress.total_steps,
                progress.title
            ));
        }
        if !state.unresolved.is_empty() {
            lines.push(String::new());
            lines.push("## Open items".into());
            for item in &state.unresolved {
                lines.push(format!("- {}: {}", item.title, item.detail));
            }
        }
    }
    lines.push(String::new());
    lines.push("_Metadata only: Knov did not read page, document, or code contents._".into());
    let content = lines
        .into_iter()
        .map(|line| sanitize_text(&line, 400))
        .collect::<Vec<_>>()
        .join("\n");
    (title, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{
        models::{CheckPresetView, SkillStepUpdate},
        workflows::{mine_workflows, score_opportunity, tests::developer_sessions},
    };

    fn workflow() -> Workflow {
        let mined = mine_workflows(&developer_sessions(4)).remove(0);
        Workflow {
            id: mined.id.clone(),
            title: mined.generated_title.clone(),
            generated_title: mined.generated_title,
            status: "confirmed".into(),
            active: true,
            opportunity: score_opportunity(&mined.steps, &mined.stats, None, false),
            steps: mined.steps,
            stats: mined.stats,
            evidence: mined.evidence,
            skill_id: None,
            first_seen_at: mined.first_seen_at,
            last_seen_at: mined.last_seen_at,
            updated_at: 0,
        }
    }

    fn workspace() -> ApprovedWorkspace {
        ApprovedWorkspace {
            id: "ws-1".into(),
            label: "knov".into(),
            path: "/Users/me/knov".into(),
            check_presets: vec![CheckPresetView {
                id: "npm-test".into(),
                label: "npm test".into(),
            }],
            created_at: 0,
        }
    }

    #[test]
    fn skills_prepare_safe_steps_and_leave_the_rest_manual() {
        let skill = skill_from_workflow(&workflow(), &[], 10);
        assert_eq!(skill.trigger.kind, "manual");
        assert_eq!(skill.on_exception, "stop");
        let types = skill
            .steps
            .iter()
            .map(|step| step.action.as_ref().map(ActionSpec::action_type))
            .collect::<Vec<_>>();
        assert_eq!(
            types,
            [
                Some("open_application"),
                Some("open_url"),
                Some("open_application"),
                Some("open_application")
            ]
        );
        assert!(matches!(
            &skill.steps[1].action,
            Some(ActionSpec::OpenUrl { resolve_domain: Some(domain), .. }) if domain == "github.com"
        ));
    }

    #[test]
    fn matching_workspace_turns_terminal_steps_into_checks() {
        let skill = skill_from_workflow(&workflow(), &[workspace()], 10);
        assert!(matches!(
            &skill.steps[3].action,
            Some(ActionSpec::RunChecks { workspace_id, preset }) if workspace_id == "ws-1" && preset == "npm-test"
        ));
        assert!(skill.steps[3].title.contains("npm test"));
    }

    #[test]
    fn updates_are_validated_and_cannot_inject_targets() {
        let skill = skill_from_workflow(&workflow(), &[], 10);
        let update = |steps: Vec<SkillStepUpdate>, trigger: SkillTrigger| SkillUpdate {
            id: skill.id.clone(),
            name: "  Morning build  ".into(),
            description: None,
            trigger,
            on_exception: "continue".into(),
            enabled: true,
            steps,
            include_brief: true,
        };
        let manual = SkillTrigger {
            kind: "manual".into(),
            weekday: None,
            hour: None,
        };
        let updated = apply_update(
            &skill,
            update(
                vec![
                    SkillStepUpdate {
                        id: "step-1".into(),
                        enabled: false,
                        workspace_id: None,
                        check_preset: None,
                    },
                    SkillStepUpdate {
                        id: "step-4".into(),
                        enabled: true,
                        workspace_id: Some("ws-1".into()),
                        check_preset: None,
                    },
                ],
                manual.clone(),
            ),
            &[workspace()],
            20,
        )
        .unwrap();
        assert_eq!(updated.name, "Morning build");
        assert!(!updated.steps[0].enabled);
        assert!(matches!(
            updated.steps[3].action,
            Some(ActionSpec::RunChecks { .. })
        ));
        assert_eq!(updated.steps.last().unwrap().id, BRIEF_STEP_ID);

        // Unknown workspaces, unknown steps, and bad triggers are rejected.
        assert!(apply_update(
            &skill,
            update(
                vec![SkillStepUpdate {
                    id: "step-4".into(),
                    enabled: true,
                    workspace_id: Some("ws-unknown".into()),
                    check_preset: None,
                }],
                manual.clone()
            ),
            &[workspace()],
            20
        )
        .is_err());
        assert!(apply_update(
            &skill,
            update(
                vec![SkillStepUpdate {
                    id: "invented".into(),
                    enabled: true,
                    workspace_id: None,
                    check_preset: None,
                }],
                manual
            ),
            &[],
            20
        )
        .is_err());
        assert!(apply_update(
            &skill,
            update(
                vec![],
                SkillTrigger {
                    kind: "schedule".into(),
                    weekday: Some(9),
                    hour: Some(9),
                }
            ),
            &[],
            20
        )
        .is_err());
    }

    #[test]
    fn drafts_are_rendered_from_sanitized_state() {
        let skill = skill_from_workflow(&workflow(), &[], 10);
        let state = WorkState {
            generated_at: 0,
            active_thread: Some("Knov desktop".into()),
            goal: None,
            goals: vec![],
            recent_steps: vec![],
            open_resources: vec![],
            workflow_progress: None,
            unresolved: vec![],
        };
        let (title, content) = render_draft("thread_brief", &skill, &state, 1_800_000_000);
        assert_eq!(title, "Resume brief: Knov desktop");
        assert!(content.contains("did not read page"));
        let (_, checklist) = render_draft("workflow_checklist", &skill, &state, 1_800_000_000);
        assert!(checklist.contains("- [ ] "));
    }
}
