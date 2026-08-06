use chrono::Utc;
use keyring::Entry;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{ChatMessage, ProfileDocument, Recommendation, RefreshResult, UserCorrection},
};

const KEYCHAIN_SERVICE: &str = "com.knov.desktop.llm";

#[derive(Clone)]
pub struct ProviderClient {
    http: Client,
}

impl Default for ProviderClient {
    fn default() -> Self {
        Self {
            http: Client::builder()
                .user_agent("Knov/0.1")
                .build()
                .expect("provider HTTP client"),
        }
    }
}

impl ProviderClient {
    pub fn save_key(&self, provider: &str, key: &str) -> AppResult<()> {
        validate_provider(provider)?;
        if key.trim().is_empty() {
            return Err(AppError::InvalidInput("API key cannot be empty".into()));
        }
        Entry::new(KEYCHAIN_SERVICE, provider)
            .map_err(|_| AppError::Credential)?
            .set_password(key.trim())
            .map_err(|_| AppError::Credential)
    }

    pub fn delete_key(&self, provider: &str) -> AppResult<()> {
        validate_provider(provider)?;
        let entry = Entry::new(KEYCHAIN_SERVICE, provider).map_err(|_| AppError::Credential)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppError::Credential),
        }
    }

    pub fn has_key(&self, provider: &str) -> bool {
        self.key(provider).is_ok()
    }

    fn key(&self, provider: &str) -> AppResult<String> {
        validate_provider(provider)?;
        if let Ok(value) = std::env::var(match provider {
            "openai" => "OPENAI_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            _ => unreachable!(),
        }) {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }
        Entry::new(KEYCHAIN_SERVICE, provider)
            .map_err(|_| AppError::Credential)?
            .get_password()
            .map_err(|error| match error {
                keyring::Error::NoEntry => AppError::ProviderNotConfigured,
                _ => AppError::Credential,
            })
    }

    pub async fn validate(&self, provider: &str) -> AppResult<()> {
        let key = self.key(provider)?;
        let result = match provider {
            "openai" => {
                self.http
                    .get("https://api.openai.com/v1/models")
                    .bearer_auth(key)
                    .send()
                    .await?
            }
            "anthropic" => {
                self.http
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&json!({
                        "model":"claude-sonnet-4-5",
                        "max_tokens":1,
                        "messages":[{"role":"user","content":"Reply OK"}]
                    }))
                    .send()
                    .await?
            }
            _ => unreachable!(),
        };
        if result.status().is_success() {
            Ok(())
        } else {
            Err(provider_status_error(result.status()))
        }
    }

    pub async fn chat(
        &self,
        provider: &str,
        profile: &ProfileDocument,
        corrections: &[UserCorrection],
        activity_summary: &Value,
        conversation: &[ChatMessage],
        message: &str,
    ) -> AppResult<String> {
        let system = format!(
            "You are Knov, a supportive personal context assistant. Never reveal raw activity logs. \
             Treat USER TRUTH as authoritative. Avoid medical/mental-health diagnosis and productivity scoring. \
             You have access to the minimized LOCAL ACTIVITY SUMMARY below. Use it when relevant and never claim \
             you lack activity access when it contains the requested app or domain. For an unspecified timeframe, \
             use 30d and state that choice. State the denominator for percentages. applicationTime and \
             liveWebsiteTime contain observed foreground durations. historicalWebsiteVisits contains visit counts \
             only; never turn those counts into time. Sustained time means foreground sessions lasting at least \
             five minutes and does not prove concentration or productivity.\n\
             PROFILE:\n{}\nUSER TRUTH:\n{}\nLOCAL ACTIVITY SUMMARY:\n{}",
            serde_json::to_string(profile)?,
            serde_json::to_string(corrections)?,
            serde_json::to_string(activity_summary)?
        );
        let mut turns = conversation.to_vec();
        turns.push(ChatMessage {
            role: "user".into(),
            content: message.into(),
        });
        self.complete(provider, &system, &turns, 1600, None).await
    }

    pub async fn refresh_profile(
        &self,
        db: &Database,
        provider: &str,
        run_kind: &str,
    ) -> AppResult<RefreshResult> {
        let now = Utc::now();
        let settings = db.settings()?;
        let since = if settings.initial_profile_completed {
            now.timestamp() - 30 * 86_400
        } else {
            now.timestamp() - 90 * 86_400
        };
        let digest = db.profile_digest(since)?;
        let corrections = db.corrections()?;
        let system = "Generate a conservative personal context profile and in-app recommendations from an aggregated digest. \
            Never diagnose health or mental state, never score productivity, never claim content was completed, and do not infer sensitive topics. \
            User truth is absolute and must be preserved. Return JSON only with keys profile and recommendations. \
            profile has summary, interests, skills, activeProjects, patterns. Each recommendation has kind, text, evidence; \
            evidence must clearly distinguish observation from inference.";
        let prompt = format!(
            "AGGREGATED ACTIVITY DIGEST:\n{}\nAUTHORITATIVE USER TRUTH:\n{}",
            digest,
            serde_json::to_string(&corrections)?
        );
        let raw = self
            .complete(
                provider,
                system,
                &[ChatMessage {
                    role: "user".into(),
                    content: prompt,
                }],
                2400,
                Some(profile_response_format()),
            )
            .await?;
        let parsed = parse_json_response(&raw)?;
        let profile_value = parsed.get("profile").cloned().ok_or_else(|| {
            AppError::Provider("The provider response did not include a profile.".into())
        })?;
        let mut profile = decode_profile(profile_value)?;
        if unsafe_guidance(&profile.summary) || sensitive_inference(&profile.summary) {
            profile.summary =
                "Profile available; sensitive or judgmental provider output was suppressed locally."
                    .into();
        }
        for values in [
            &mut profile.interests,
            &mut profile.skills,
            &mut profile.active_projects,
            &mut profile.patterns,
        ] {
            values.retain(|value| {
                !unsafe_guidance(value)
                    && !sensitive_inference(value)
                    && !conflicts_with_truth(value, &corrections)
            });
        }
        profile.updated_at = now.timestamp();
        // Corrections are intentionally not folded into inferred arrays: storing them separately
        // guarantees regeneration cannot overwrite user truth.
        let recommendations = parsed
            .get("recommendations")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| {
                let object = value.as_object()?;
                let text = object.get("text")?.as_str()?.to_string();
                let evidence = object.get("evidence")?.as_str()?.to_string();
                if unsafe_guidance(&text) || unsafe_guidance(&evidence) {
                    return None;
                }
                let kind = object.get("kind")?.as_str()?.to_string();
                if kind == "behavioral" && !settings.behavioral_guidance_enabled {
                    return None;
                }
                Some(Recommendation {
                    id: Uuid::new_v4().to_string(),
                    kind,
                    text,
                    evidence,
                    dismissed: false,
                    feedback: None,
                    created_at: now.timestamp(),
                })
            })
            .take(8)
            .collect::<Vec<_>>();
        let run_day = now.format("%Y-%m-%d").to_string();
        let was_initial = !settings.initial_profile_completed;
        let mut updated = settings;
        if was_initial {
            updated.initial_profile_completed = true;
        }
        updated.last_profile_refresh_day = Some(run_day.clone());
        db.commit_profile_refresh(
            &profile,
            &recommendations,
            &updated,
            &run_day,
            run_kind,
            was_initial.then_some(now.timestamp() - 30 * 86_400),
        )?;
        Ok(RefreshResult {
            profile,
            recommendations,
            completed_at: now.timestamp(),
        })
    }

    async fn complete(
        &self,
        provider: &str,
        system: &str,
        messages: &[ChatMessage],
        max_tokens: u32,
        response_format: Option<Value>,
    ) -> AppResult<String> {
        let key = self.key(provider)?;
        match provider {
            "openai" => {
                let mut input = vec![json!({"role":"developer","content":system})];
                input.extend(messages.iter().map(|m| {
                    json!({"role": if m.role == "assistant" {"assistant"} else {"user"}, "content":m.content})
                }));
                let mut request = json!({
                    "model":"gpt-5-mini",
                    "input":input,
                    "max_output_tokens":max_tokens,
                    "store":false
                });
                if let Some(format) = response_format {
                    request["text"] = json!({"format":format});
                }
                let response = self
                    .http
                    .post("https://api.openai.com/v1/responses")
                    .bearer_auth(key)
                    .json(&request)
                    .send()
                    .await?;
                let status = response.status();
                let body: Value = response.json().await?;
                if !status.is_success() {
                    return Err(provider_status_error(status));
                }
                body.get("output")
                    .and_then(|v| v.as_array())
                    .and_then(|items| {
                        items.iter().find_map(|item| {
                            item.get("content")?.as_array()?.iter().find_map(|content| {
                                (content.get("type")?.as_str()? == "output_text")
                                    .then(|| content.get("text")?.as_str().map(ToOwned::to_owned))
                                    .flatten()
                            })
                        })
                    })
                    .ok_or_else(|| AppError::Provider("The provider returned no text.".into()))
            }
            "anthropic" => {
                let response = self
                    .http
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&json!({
                        "model":"claude-sonnet-4-5",
                        "system":system,
                        "max_tokens":max_tokens,
                        "messages":messages.iter().map(|m| json!({
                            "role": if m.role == "assistant" {"assistant"} else {"user"},
                            "content":m.content
                        })).collect::<Vec<_>>()
                    }))
                    .send()
                    .await?;
                let status = response.status();
                let body: Value = response.json().await?;
                if !status.is_success() {
                    return Err(provider_status_error(status));
                }
                body.get("content")
                    .and_then(|v| v.as_array())
                    .and_then(|items| {
                        items.iter().find_map(|item| {
                            (item.get("type")?.as_str()? == "text")
                                .then(|| item.get("text")?.as_str().map(ToOwned::to_owned))
                                .flatten()
                        })
                    })
                    .ok_or_else(|| AppError::Provider("The provider returned no text.".into()))
            }
            _ => Err(AppError::InvalidInput("Unsupported provider.".into())),
        }
    }
}

fn validate_provider(provider: &str) -> AppResult<()> {
    match provider {
        "openai" | "anthropic" => Ok(()),
        _ => Err(AppError::InvalidInput("Unsupported provider.".into())),
    }
}

fn provider_status_error(status: StatusCode) -> AppError {
    let message = match status.as_u16() {
        401 | 403 => "The API key is invalid or revoked.",
        402 => "The provider account has no available credit.",
        429 => "The provider rate limit or quota was reached.",
        500..=599 => "The provider is temporarily unavailable.",
        _ => "The provider rejected the request.",
    };
    AppError::Provider(message.into())
}

fn strip_json_fence(value: &str) -> &str {
    value
        .trim()
        .strip_prefix("```json")
        .and_then(|v| v.strip_suffix("```"))
        .unwrap_or(value)
        .trim()
}

fn parse_json_response(value: &str) -> AppResult<Value> {
    let trimmed = strip_json_fence(value);
    if let Ok(parsed) = serde_json::from_str(trimmed) {
        return Ok(parsed);
    }

    let object_start = trimmed.find('{').ok_or_else(invalid_profile_format)?;
    serde_json::Deserializer::from_str(&trimmed[object_start..])
        .into_iter::<Value>()
        .next()
        .transpose()
        .map_err(|_| invalid_profile_format())?
        .ok_or_else(invalid_profile_format)
}

fn invalid_profile_format() -> AppError {
    AppError::Provider("The provider returned an invalid profile format.".into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratedProfile {
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    interests: Option<Vec<String>>,
    #[serde(default)]
    skills: Option<Vec<String>>,
    #[serde(default, alias = "active_projects")]
    active_projects: Option<Vec<String>>,
    #[serde(default)]
    patterns: Option<Vec<String>>,
}

fn decode_profile(value: Value) -> AppResult<ProfileDocument> {
    let generated: GeneratedProfile =
        serde_json::from_value(value).map_err(|_| invalid_profile_format())?;
    Ok(ProfileDocument {
        summary: generated.summary.unwrap_or_default(),
        interests: generated.interests.unwrap_or_default(),
        skills: generated.skills.unwrap_or_default(),
        active_projects: generated.active_projects.unwrap_or_default(),
        patterns: generated.patterns.unwrap_or_default(),
        updated_at: 0,
    })
}

fn profile_response_format() -> Value {
    json!({
        "type":"json_schema",
        "name":"knov_profile_refresh",
        "strict":true,
        "schema":{
            "type":"object",
            "properties":{
                "profile":{
                    "type":"object",
                    "properties":{
                        "summary":{"type":"string"},
                        "interests":{"type":"array","items":{"type":"string"}},
                        "skills":{"type":"array","items":{"type":"string"}},
                        "activeProjects":{"type":"array","items":{"type":"string"}},
                        "patterns":{"type":"array","items":{"type":"string"}}
                    },
                    "required":["summary","interests","skills","activeProjects","patterns"],
                    "additionalProperties":false
                },
                "recommendations":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{
                            "kind":{"type":"string"},
                            "text":{"type":"string"},
                            "evidence":{"type":"string"}
                        },
                        "required":["kind","text","evidence"],
                        "additionalProperties":false
                    }
                }
            },
            "required":["profile","recommendations"],
            "additionalProperties":false
        }
    })
}

fn unsafe_guidance(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "diagnos",
        "addict",
        "mental illness",
        "depress",
        "anxiety disorder",
        "unproductive",
        "lazy",
        "productivity score",
    ]
    .iter()
    .any(|term| normalized.contains(term))
}

fn sensitive_inference(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "medical",
        "health condition",
        "financial status",
        "banking",
        "religion",
        "religious belief",
        "political affiliation",
        "sexual orientation",
    ]
    .iter()
    .any(|term| normalized.contains(term))
}

fn conflicts_with_truth(value: &str, corrections: &[UserCorrection]) -> bool {
    let inferred = value.to_ascii_lowercase();
    corrections.iter().any(|correction| {
        let truth = format!("{} {}", correction.subject, correction.value).to_ascii_lowercase();
        if truth.contains(&inferred) {
            return true;
        }
        let corrective = [
            "no longer",
            "not ",
            "stopped",
            "finished",
            "complete",
            "incorrect",
        ]
        .iter()
        .any(|marker| truth.contains(marker));
        corrective
            && inferred
                .split(|character: char| !character.is_ascii_alphanumeric())
                .filter(|token| token.len() >= 4)
                .any(|token| truth.contains(token))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_json_wrapped_in_provider_commentary() {
        let parsed = parse_json_response(
            "Here is the requested profile:\n{\"profile\":{\"summary\":\"Focused\"},\"recommendations\":[]}\nDone.",
        )
        .expect("embedded JSON should be accepted");

        assert_eq!(parsed["profile"]["summary"], "Focused");
    }

    #[test]
    fn rejects_response_without_json() {
        assert!(matches!(
            parse_json_response("I could not generate a profile."),
            Err(AppError::Provider(_))
        ));
    }

    #[test]
    fn decodes_profile_with_nullable_optional_collections() {
        let profile = decode_profile(json!({
            "summary": "Focused",
            "interests": null,
            "skills": ["Rust"],
            "activeProjects": null,
            "patterns": []
        }))
        .expect("nullable provider fields should use safe empty defaults");

        assert_eq!(profile.summary, "Focused");
        assert!(profile.interests.is_empty());
        assert_eq!(profile.skills, ["Rust"]);
        assert!(profile.active_projects.is_empty());
    }

    #[test]
    fn decodes_snake_case_active_projects_from_prompt_only_providers() {
        let profile = decode_profile(json!({
            "summary": "Focused",
            "interests": [],
            "skills": [],
            "active_projects": ["Knov"],
            "patterns": []
        }))
        .expect("snake_case provider output should be accepted");

        assert_eq!(profile.active_projects, ["Knov"]);
    }

    #[test]
    fn blocks_judgmental_or_diagnostic_guidance() {
        assert!(unsafe_guidance("You seem unproductive today."));
        assert!(unsafe_guidance("This may diagnose anxiety disorder."));
        assert!(!unsafe_guidance(
            "You had a long observed session; consider a break."
        ));
        assert!(sensitive_inference(
            "The user may have a particular health condition."
        ));
        assert!(conflicts_with_truth(
            "Project Atlas",
            &[UserCorrection {
                id: "truth".into(),
                subject: "I am no longer working on Project Atlas".into(),
                value: "It is complete".into(),
                created_at: 0,
                updated_at: 0,
            }]
        ));
    }
}
