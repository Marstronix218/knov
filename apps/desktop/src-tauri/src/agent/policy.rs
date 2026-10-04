//! Permission policy. Confidence never grants authority: an action runs
//! automatically only under an explicit, unexpired, unrevoked user grant whose
//! scope matches, and only while the agent is not paused.

use super::{
    actions::{ActionKind, RiskClass},
    models::AutonomyGrant,
};

/// Approvals required before Knov suggests broader permission.
pub(crate) const PROPOSAL_MIN_APPROVALS: i64 = 5;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Decision {
    Allow {
        grant_id: Option<String>,
        reason: String,
    },
    Ask {
        reason: String,
    },
    Deny {
        reason: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ActionScope {
    pub skill_id: Option<String>,
    pub workspace_id: Option<String>,
}

impl ActionScope {
    /// The scope a "remember this" approval or a proposal applies to: the
    /// workspace for checks, otherwise the skill.
    pub(crate) fn primary(&self, action_type: &str) -> (&'static str, Option<String>) {
        if action_type == "run_checks" {
            if let Some(workspace) = &self.workspace_id {
                return ("workspace", Some(workspace.clone()));
            }
        }
        match &self.skill_id {
            Some(skill) => ("skill", Some(skill.clone())),
            None => ("global", None),
        }
    }
}

pub(crate) fn grant_matches(grant: &AutonomyGrant, action_type: &str, scope: &ActionScope) -> bool {
    grant.action_type == action_type
        && match grant.scope_kind.as_str() {
            "global" => true,
            "skill" => grant.scope_value.is_some() && grant.scope_value == scope.skill_id,
            "workspace" => grant.scope_value.is_some() && grant.scope_value == scope.workspace_id,
            _ => false,
        }
}

fn specificity(scope_kind: &str) -> u8 {
    match scope_kind {
        "skill" => 3,
        "workspace" => 2,
        _ => 1,
    }
}

/// `grants` must already exclude revoked and expired grants.
pub(crate) fn authorize(
    kind: &ActionKind,
    scope: &ActionScope,
    grants: &[AutonomyGrant],
    paused: bool,
    unattended: bool,
) -> Decision {
    if paused {
        return Decision::Deny {
            reason: "Agent execution is paused. Resume it to let Knov act.".into(),
        };
    }
    if !kind.available || kind.risk >= RiskClass::ExternalCommunication {
        return Decision::Deny {
            reason: format!("“{}” is not available in this alpha.", kind.title),
        };
    }
    let matching = grants
        .iter()
        .filter(|grant| grant_matches(grant, kind.id, scope))
        .collect::<Vec<_>>();
    if matching.iter().any(|grant| grant.mode == "never") {
        return Decision::Deny {
            reason: "You chose never to allow this here.".into(),
        };
    }
    let best = matching
        .into_iter()
        .max_by_key(|grant| specificity(&grant.scope_kind));
    match best {
        Some(grant) if grant.mode == "auto" => {
            if unattended && kind.interrupts_user {
                Decision::Ask {
                    reason: "Background runs never open windows or apps on their own; approve to stage it now."
                        .into(),
                }
            } else {
                Decision::Allow {
                    grant_id: Some(grant.id.clone()),
                    reason: format!("Allowed by your grant for {}.", grant.scope_label),
                }
            }
        }
        Some(_) => Decision::Ask {
            reason: "Your permission for this says to ask first.".into(),
        },
        None if kind.risk == RiskClass::ReadOnly => Decision::Allow {
            grant_id: None,
            reason: "Read-only actions do not need a grant.".into(),
        },
        None => Decision::Ask {
            reason: "Knov asks before acting until you allow this kind of action.".into(),
        },
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ApprovalHistory {
    pub approvals: i64,
    pub rejections: i64,
    pub failures: i64,
    pub rolled_back: i64,
}

/// Successful repeated approval earns a permission *proposal*; it never
/// expands authority by itself.
pub(crate) fn proposal_eligible(
    history: &ApprovalHistory,
    has_grant: bool,
    approvals_at_dismissal: Option<i64>,
) -> bool {
    history.approvals >= PROPOSAL_MIN_APPROVALS
        && history.rejections == 0
        && history.failures == 0
        && history.rolled_back == 0
        && !has_grant
        && approvals_at_dismissal.is_none_or(|previous| history.approvals >= previous * 2)
}

pub(crate) fn tendency(approvals: i64, rejections: i64, automatic: i64) -> &'static str {
    if automatic > 0 && rejections == 0 {
        "Runs automatically"
    } else if approvals + rejections == 0 {
        "Not decided yet"
    } else if rejections == 0 {
        "You usually approve"
    } else if approvals == 0 {
        "You usually decline"
    } else if approvals >= rejections * 3 {
        "Mostly approved"
    } else {
        "Mixed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::actions::action_kind;

    fn grant(id: &str, scope_kind: &str, scope_value: Option<&str>, mode: &str) -> AutonomyGrant {
        AutonomyGrant {
            id: id.into(),
            action_type: "open_url".into(),
            scope_kind: scope_kind.into(),
            scope_value: scope_value.map(str::to_string),
            scope_label: "scope".into(),
            mode: mode.into(),
            source: "user".into(),
            created_at: 0,
            expires_at: None,
        }
    }

    fn scope() -> ActionScope {
        ActionScope {
            skill_id: Some("skill-1".into()),
            workspace_id: None,
        }
    }

    #[test]
    fn default_is_ask_and_pause_or_unavailable_always_deny() {
        let open = action_kind("open_url").unwrap();
        assert!(matches!(
            authorize(open, &scope(), &[], false, false),
            Decision::Ask { .. }
        ));
        let auto = [grant("g", "global", None, "auto")];
        assert!(matches!(
            authorize(open, &scope(), &auto, true, false),
            Decision::Deny { .. }
        ));
        let send = action_kind("send_message").unwrap();
        let send_grant = [AutonomyGrant {
            action_type: "send_message".into(),
            ..grant("s", "global", None, "auto")
        }];
        assert!(matches!(
            authorize(send, &scope(), &send_grant, false, false),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn never_overrides_auto_and_specific_scope_wins() {
        let open = action_kind("open_url").unwrap();
        let grants = [
            grant("global-auto", "global", None, "auto"),
            grant("skill-ask", "skill", Some("skill-1"), "ask"),
        ];
        assert!(matches!(
            authorize(open, &scope(), &grants, false, false),
            Decision::Ask { .. }
        ));
        let other_skill = ActionScope {
            skill_id: Some("skill-2".into()),
            workspace_id: None,
        };
        assert!(matches!(
            authorize(open, &other_skill, &grants, false, false),
            Decision::Allow { grant_id: Some(ref id), .. } if id == "global-auto"
        ));
        let with_never = [
            grant("skill-auto", "skill", Some("skill-1"), "auto"),
            grant("global-never", "global", None, "never"),
        ];
        assert!(matches!(
            authorize(open, &scope(), &with_never, false, false),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn unattended_runs_never_steal_focus_even_with_a_grant() {
        let open = action_kind("open_url").unwrap();
        let grants = [grant("g", "skill", Some("skill-1"), "auto")];
        assert!(matches!(
            authorize(open, &scope(), &grants, false, true),
            Decision::Ask { .. }
        ));
        let draft = action_kind("write_draft").unwrap();
        let draft_grant = [AutonomyGrant {
            action_type: "write_draft".into(),
            ..grant("d", "skill", Some("skill-1"), "auto")
        }];
        assert!(matches!(
            authorize(draft, &scope(), &draft_grant, false, true),
            Decision::Allow { .. }
        ));
    }

    #[test]
    fn proposals_require_clean_repeated_approvals_and_respect_dismissal() {
        let clean = ApprovalHistory {
            approvals: 5,
            ..ApprovalHistory::default()
        };
        assert!(proposal_eligible(&clean, false, None));
        assert!(!proposal_eligible(&clean, true, None));
        assert!(!proposal_eligible(
            &ApprovalHistory {
                rejections: 1,
                ..clean.clone()
            },
            false,
            None
        ));
        assert!(!proposal_eligible(&clean, false, Some(5)));
        assert!(proposal_eligible(
            &ApprovalHistory {
                approvals: 10,
                ..clean
            },
            false,
            Some(5)
        ));
    }
}
