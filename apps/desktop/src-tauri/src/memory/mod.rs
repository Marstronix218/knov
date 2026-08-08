use std::collections::HashSet;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{ProfileDocument, UserCorrection};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecord {
    pub id: String,
    pub text: String,
    pub memory_type: String,
    pub source: String,
    pub created_at: i64,
    pub importance: Option<f64>,
    pub score: Option<f64>,
}

pub fn approved_profile_memories(
    profile: &ProfileDocument,
    corrections: &[UserCorrection],
) -> Vec<MemoryRecord> {
    let now = Utc::now().timestamp();
    let mut values = Vec::new();
    if !profile.summary.trim().is_empty() {
        values.push(memory("profile", "local_profile", &profile.summary, now));
    }
    for value in &profile.interests {
        values.push(memory("interest", "local_profile", value, now));
    }
    for value in &profile.active_projects {
        values.push(memory("project", "local_profile", value, now));
    }
    for value in &profile.patterns {
        values.push(memory("pattern", "local_profile", value, now));
    }
    for correction in corrections {
        let text = if correction.value.trim().is_empty() {
            correction.subject.clone()
        } else {
            format!("{} — {}", correction.subject, correction.value)
        };
        values.push(memory(
            "preference",
            "explicit_user",
            &text,
            correction.updated_at,
        ));
    }
    values
}

pub fn safe_local_search(
    profile: &ProfileDocument,
    corrections: &[UserCorrection],
    query: &str,
    limit: usize,
) -> Vec<MemoryRecord> {
    let query_terms = terms(query);
    let mut memories = approved_profile_memories(profile, corrections)
        .into_iter()
        .map(|mut memory| {
            let memory_terms = terms(&memory.text);
            let overlap = query_terms.intersection(&memory_terms).count() as f64;
            let authoritative = (memory.source == "explicit_user") as u8 as f64;
            memory.score = Some((overlap * 0.2 + authoritative * 0.3).min(1.0));
            memory
        })
        .collect::<Vec<_>>();
    memories.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    memories.truncate(limit);
    memories
}

fn memory(memory_type: &str, source: &str, text: &str, created_at: i64) -> MemoryRecord {
    MemoryRecord {
        id: Uuid::new_v4().to_string(),
        text: text.trim().into(),
        memory_type: memory_type.into(),
        source: source.into(),
        created_at,
        importance: None,
        score: None,
    }
}

fn terms(value: &str) -> HashSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.len() > 2)
        .map(str::to_ascii_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProfileDocument;

    #[test]
    fn local_search_prioritizes_authoritative_corrections() {
        let profile = ProfileDocument {
            summary: "Enjoys building private personal software.".into(),
            interests: vec!["local-first systems".into()],
            skills: Vec::new(),
            active_projects: Vec::new(),
            patterns: Vec::new(),
            updated_at: 0,
        };
        let corrections = vec![UserCorrection {
            id: "correction-1".into(),
            subject: "Privacy priority".into(),
            value: "Privacy matters more than feature count.".into(),
            created_at: 1,
            updated_at: 2,
        }];

        let result = safe_local_search(&profile, &corrections, "What privacy priority matters?", 1);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, "explicit_user");
    }
}
