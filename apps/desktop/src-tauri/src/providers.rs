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

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub text: String,
    pub model: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub preflight_input_tokens: Option<i64>,
    pub cache_read_input_tokens: Option<i64>,
    pub cache_write_input_tokens: Option<i64>,
}

#[derive(Clone)]
pub struct ProviderClient {
    http: Client,
}

impl Default for ProviderClient {
    fn default() -> Self {
        Self {
            http: Client::builder()
                .user_agent("Knov/0.2")
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
            "bedrock" => "AWS_BEDROCK_API_KEY",
            _ => unreachable!(),
        }) {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }
        if provider == "bedrock" {
            if let Ok(value) = std::env::var("AWS_BEARER_TOKEN_BEDROCK") {
                if !value.trim().is_empty() {
                    return Ok(value);
                }
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
            "bedrock" => {
                let model = bedrock_model();
                let validation_message = [ChatMessage {
                    role: "user".into(),
                    content: "Reply OK".into(),
                }];
                self.http
                    .post(bedrock_url(&model, "count-tokens")?)
                    .bearer_auth(key)
                    .json(&bedrock_count_tokens_request(
                        "Validate the configured Amazon Bedrock credential.",
                        &validation_message,
                        false,
                    ))
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
        system: &str,
        conversation: &[ChatMessage],
        message: &str,
        input_token_limit: i64,
    ) -> AppResult<CompletionResult> {
        let mut turns = conversation.to_vec();
        turns.push(ChatMessage {
            role: "user".into(),
            content: message.into(),
        });
        self.complete(
            provider,
            system,
            &turns,
            1600,
            None,
            Some(input_token_limit),
        )
        .await
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
                None,
            )
            .await?;
        let parsed = parse_json_response(&raw.text)?;
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
        input_token_limit: Option<i64>,
    ) -> AppResult<CompletionResult> {
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
                let text = body
                    .get("output")
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
                    .ok_or_else(|| AppError::Provider("The provider returned no text.".into()))?;
                Ok(CompletionResult {
                    text,
                    model: body["model"].as_str().unwrap_or("gpt-5-mini").into(),
                    input_tokens: body["usage"]["input_tokens"].as_i64(),
                    output_tokens: body["usage"]["output_tokens"].as_i64(),
                    preflight_input_tokens: None,
                    cache_read_input_tokens: None,
                    cache_write_input_tokens: None,
                })
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
                let text = body
                    .get("content")
                    .and_then(|v| v.as_array())
                    .and_then(|items| {
                        items.iter().find_map(|item| {
                            (item.get("type")?.as_str()? == "text")
                                .then(|| item.get("text")?.as_str().map(ToOwned::to_owned))
                                .flatten()
                        })
                    })
                    .ok_or_else(|| AppError::Provider("The provider returned no text.".into()))?;
                Ok(CompletionResult {
                    text,
                    model: body["model"].as_str().unwrap_or("claude-sonnet-4-5").into(),
                    input_tokens: body["usage"]["input_tokens"].as_i64(),
                    output_tokens: body["usage"]["output_tokens"].as_i64(),
                    preflight_input_tokens: None,
                    cache_read_input_tokens: body["usage"]["cache_read_input_tokens"].as_i64(),
                    cache_write_input_tokens: body["usage"]["cache_creation_input_tokens"].as_i64(),
                })
            }
            "bedrock" => {
                let model = bedrock_model();
                let cache_prompt = bedrock_prompt_cache_enabled(system, &model);
                let count_response = self
                    .http
                    .post(bedrock_url(&model, "count-tokens")?)
                    .bearer_auth(&key)
                    .json(&bedrock_count_tokens_request(
                        system,
                        messages,
                        cache_prompt,
                    ))
                    .send()
                    .await?;
                let count_status = count_response.status();
                let count_body: Value = count_response.json().await?;
                if !count_status.is_success() {
                    return Err(provider_status_error(count_status));
                }
                let preflight_input_tokens = bedrock_preflight_tokens(&count_body)?;
                if input_token_limit.is_some_and(|limit| preflight_input_tokens > limit) {
                    return Err(AppError::InvalidInput(
                        "The exact Amazon Bedrock prompt count exceeded the configured request budget; shorten the question or reduce selected context."
                            .into(),
                    ));
                }

                let response = self
                    .http
                    .post(bedrock_url(&model, "converse")?)
                    .bearer_auth(key)
                    .json(&bedrock_converse_request(
                        system,
                        messages,
                        max_tokens,
                        cache_prompt,
                    ))
                    .send()
                    .await?;
                let status = response.status();
                let body: Value = response.json().await?;
                if !status.is_success() {
                    return Err(provider_status_error(status));
                }
                bedrock_completion(&body, model, Some(preflight_input_tokens))
            }
            _ => Err(AppError::InvalidInput("Unsupported provider.".into())),
        }
    }
}

fn validate_provider(provider: &str) -> AppResult<()> {
    match provider {
        "openai" | "anthropic" | "bedrock" => Ok(()),
        _ => Err(AppError::InvalidInput("Unsupported provider.".into())),
    }
}

fn bedrock_region() -> String {
    std::env::var("AWS_BEDROCK_REGION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "us-east-1".into())
}

fn bedrock_model() -> String {
    std::env::var("AWS_BEDROCK_MODEL_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "us.anthropic.claude-sonnet-4-6".into())
}

fn bedrock_url(model: &str, operation: &str) -> AppResult<reqwest::Url> {
    let mut url = reqwest::Url::parse(&format!(
        "https://bedrock-runtime.{}.amazonaws.com",
        bedrock_region()
    ))
    .map_err(|_| AppError::InvalidInput("Invalid Amazon Bedrock region.".into()))?;
    url.path_segments_mut()
        .map_err(|_| AppError::InvalidInput("Invalid Amazon Bedrock endpoint.".into()))?
        .extend(["model", model, operation]);
    Ok(url)
}

fn bedrock_messages(messages: &[ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            json!({
                "role": if message.role == "assistant" { "assistant" } else { "user" },
                "content": [{"text": message.content}]
            })
        })
        .collect()
}

fn bedrock_converse_request(
    system: &str,
    messages: &[ChatMessage],
    max_tokens: u32,
    cache_prompt: bool,
) -> Value {
    let mut system_blocks = vec![json!({"text": system})];
    if cache_prompt {
        system_blocks.push(json!({"cachePoint": {"type": "default"}}));
    }
    json!({
        "system": system_blocks,
        "messages": bedrock_messages(messages),
        "inferenceConfig": {"maxTokens": max_tokens}
    })
}

fn bedrock_count_tokens_request(
    system: &str,
    messages: &[ChatMessage],
    cache_prompt: bool,
) -> Value {
    let mut system_blocks = vec![json!({"text": system})];
    if cache_prompt {
        system_blocks.push(json!({"cachePoint": {"type": "default"}}));
    }
    json!({
        "input": {
            "converse": {
                "system": system_blocks,
                "messages": bedrock_messages(messages)
            }
        }
    })
}

fn bedrock_response_text(body: &Value) -> Option<String> {
    body.get("output")?
        .get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .find_map(|block| block.get("text")?.as_str().map(ToOwned::to_owned))
}

fn bedrock_preflight_tokens(body: &Value) -> AppResult<i64> {
    body["inputTokens"].as_i64().ok_or_else(|| {
        AppError::Provider("Amazon Bedrock returned an invalid CountTokens response.".into())
    })
}

fn bedrock_completion(
    body: &Value,
    model: String,
    preflight_input_tokens: Option<i64>,
) -> AppResult<CompletionResult> {
    let text = bedrock_response_text(body)
        .ok_or_else(|| AppError::Provider("The provider returned no text.".into()))?;
    Ok(CompletionResult {
        text,
        model,
        input_tokens: body["usage"]["inputTokens"].as_i64(),
        output_tokens: body["usage"]["outputTokens"].as_i64(),
        preflight_input_tokens,
        cache_read_input_tokens: body["usage"]["cacheReadInputTokens"].as_i64(),
        cache_write_input_tokens: body["usage"]["cacheWriteInputTokens"].as_i64(),
    })
}

fn bedrock_prompt_cache_enabled(system: &str, model: &str) -> bool {
    let configured_minimum = std::env::var("AWS_BEDROCK_CACHE_MIN_TOKENS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok());
    let documented_minimum = if model.contains("anthropic.claude-sonnet-4-6") {
        Some(1_024)
    } else {
        configured_minimum
    };
    let Some(minimum) = documented_minimum else {
        return false;
    };
    if estimated_prompt_tokens(system) < minimum {
        return false;
    }
    match std::env::var("AWS_BEDROCK_ENABLE_PROMPT_CACHE") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => true,
    }
}

fn estimated_prompt_tokens(value: &str) -> usize {
    value.chars().count().div_ceil(4)
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

    #[test]
    fn builds_bedrock_converse_request_with_cache_point() {
        let request = bedrock_converse_request(
            "Stable context",
            &[
                ChatMessage {
                    role: "user".into(),
                    content: "What changed?".into(),
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: "The query changed.".into(),
                },
            ],
            900,
            true,
        );

        assert_eq!(request["system"][0]["text"], "Stable context");
        assert_eq!(request["system"][1]["cachePoint"]["type"], "default");
        assert_eq!(request["messages"][0]["role"], "user");
        assert_eq!(
            request["messages"][0]["content"][0]["text"],
            "What changed?"
        );
        assert_eq!(request["messages"][1]["role"], "assistant");
        assert_eq!(request["inferenceConfig"]["maxTokens"], 900);
        assert!(validate_provider("bedrock").is_ok());

        let uncached = bedrock_converse_request("Context", &[], 100, false);
        assert_eq!(uncached["system"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn builds_bedrock_count_tokens_request_without_inference_config() {
        let request = bedrock_count_tokens_request(
            "Context",
            &[ChatMessage {
                role: "user".into(),
                content: "Question".into(),
            }],
            true,
        );

        assert_eq!(request["input"]["converse"]["system"][0]["text"], "Context");
        assert_eq!(
            request["input"]["converse"]["system"][1]["cachePoint"]["type"],
            "default"
        );
        assert_eq!(
            request["input"]["converse"]["messages"][0]["content"][0]["text"],
            "Question"
        );
        assert!(request["input"]["converse"]
            .get("inferenceConfig")
            .is_none());
    }

    #[test]
    fn parses_bedrock_converse_text_response() {
        let body = json!({
            "output": {
                "message": {
                    "role": "assistant",
                    "content": [
                        {"reasoningContent": {"reasoningText": {"text": "internal"}}},
                        {"text": "Visible answer"}
                    ]
                }
            },
            "usage": {
                "inputTokens": 1400,
                "outputTokens": 72,
                "cacheReadInputTokens": 1024,
                "cacheWriteInputTokens": 376
            }
        });

        let completion = bedrock_completion(&body, "test-model".into(), Some(1_398))
            .expect("valid Converse response should parse");
        assert_eq!(completion.text, "Visible answer");
        assert_eq!(completion.model, "test-model");
        assert_eq!(completion.preflight_input_tokens, Some(1_398));
        assert_eq!(completion.input_tokens, Some(1_400));
        assert_eq!(completion.output_tokens, Some(72));
        assert_eq!(completion.cache_read_input_tokens, Some(1_024));
        assert_eq!(completion.cache_write_input_tokens, Some(376));
        assert_eq!(estimated_prompt_tokens(&"x".repeat(4_096)), 1_024);
    }

    #[test]
    fn bedrock_preflight_requires_a_numeric_token_count() {
        assert_eq!(
            bedrock_preflight_tokens(&json!({"inputTokens": 42})).unwrap(),
            42
        );
        assert!(bedrock_preflight_tokens(&json!({})).is_err());
        assert!(bedrock_preflight_tokens(&json!({"inputTokens": "42"})).is_err());
    }
}
