use std::env;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMeasurement {
    pub baseline_input_tokens: i64,
    pub optimized_input_tokens: i64,
    pub measurement_method: String,
}

impl TokenMeasurement {
    pub fn tokens_saved(&self) -> i64 {
        (self.baseline_input_tokens - self.optimized_input_tokens).max(0)
    }

    pub fn reduction_percent(&self) -> f64 {
        if self.baseline_input_tokens <= 0 {
            0.0
        } else {
            self.tokens_saved() as f64 * 100.0 / self.baseline_input_tokens as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceRun {
    pub id: String,
    pub timestamp: String,
    pub model: String,
    pub baseline_input_tokens: i64,
    pub optimized_input_tokens: i64,
    pub tokens_saved: i64,
    pub reduction_percent: f64,
    pub actual_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub context_budget_tokens: i64,
    pub context_estimated_tokens: i64,
    pub context_units_considered: i64,
    pub context_units_sent: i64,
    pub context_units_omitted: i64,
    pub context_detail_level: String,
    pub provider_preflight_input_tokens: Option<i64>,
    pub cache_read_input_tokens: Option<i64>,
    pub cache_write_input_tokens: Option<i64>,
    pub latency_ms: i64,
    pub estimated_cost_usd: Option<f64>,
    pub memory_count: i64,
    pub mode: String,
    pub memory_provider: String,
    pub measurement_method: String,
}

pub fn local_token_measurement(baseline: &str, optimized: &str) -> TokenMeasurement {
    TokenMeasurement {
        baseline_input_tokens: estimated_tokens(baseline),
        optimized_input_tokens: estimated_tokens(optimized),
        measurement_method: "local_character_estimate".into(),
    }
}

pub fn provider_scaled_measurement(
    baseline: &str,
    optimized: &str,
    actual_input_tokens: Option<i64>,
    mode: &str,
) -> TokenMeasurement {
    let local = local_token_measurement(baseline, optimized);
    let Some(actual) = actual_input_tokens.filter(|value| *value > 0) else {
        return local;
    };
    let (baseline_input_tokens, optimized_input_tokens) = if mode == "baseline" {
        let ratio = local.optimized_input_tokens as f64 / local.baseline_input_tokens.max(1) as f64;
        (actual, (actual as f64 * ratio).round().max(1.0) as i64)
    } else {
        let ratio = local.baseline_input_tokens as f64 / local.optimized_input_tokens.max(1) as f64;
        ((actual as f64 * ratio).round().max(1.0) as i64, actual)
    };
    TokenMeasurement {
        baseline_input_tokens,
        optimized_input_tokens,
        measurement_method: "provider_usage_scaled_estimate".into(),
    }
}

pub fn estimated_tokens(value: &str) -> i64 {
    let mut prose_chars = 0_i64;
    let mut dense_chars = 0_i64;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character.is_ascii_whitespace() {
            prose_chars += 1;
        } else if character.is_ascii() {
            dense_chars += 1;
        } else {
            dense_chars += 2;
        }
    }
    (((prose_chars + 3) / 4) + ((dense_chars + 1) / 2)).max(1)
}

pub fn estimated_cost_usd(
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_input_tokens: Option<i64>,
    cache_write_input_tokens: Option<i64>,
) -> Option<f64> {
    let input_rate = non_empty_env("KNOV_INPUT_COST_PER_MILLION")?
        .parse::<f64>()
        .ok()?;
    let output_rate = non_empty_env("KNOV_OUTPUT_COST_PER_MILLION")?
        .parse::<f64>()
        .ok()?;
    let cache_read_rate = non_empty_env("KNOV_CACHE_READ_COST_PER_MILLION")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(input_rate);
    let cache_write_rate = non_empty_env("KNOV_CACHE_WRITE_COST_PER_MILLION")
        .and_then(|value| value.parse::<f64>().ok());
    if cache_write_input_tokens.unwrap_or_default() > 0 && cache_write_rate.is_none() {
        return None;
    }
    Some(
        input_tokens.unwrap_or_default() as f64 * input_rate / 1_000_000.0
            + cache_read_input_tokens.unwrap_or_default() as f64 * cache_read_rate / 1_000_000.0
            + cache_write_input_tokens.unwrap_or_default() as f64
                * cache_write_rate.unwrap_or(input_rate)
                / 1_000_000.0
            + output_tokens.unwrap_or_default() as f64 * output_rate / 1_000_000.0,
    )
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_measurement_is_consistent_and_never_negative() {
        let measurement = local_token_measurement("abcdefgh", "abcd");
        assert_eq!(measurement.baseline_input_tokens, 2);
        assert_eq!(measurement.optimized_input_tokens, 1);
        assert_eq!(measurement.tokens_saved(), 1);
        assert_eq!(measurement.reduction_percent(), 50.0);
    }

    #[test]
    fn provider_usage_scales_the_unsent_comparison_prompt() {
        let measurement = provider_scaled_measurement("abcdefgh", "abcd", Some(10), "optimized");
        assert_eq!(measurement.baseline_input_tokens, 20);
        assert_eq!(measurement.optimized_input_tokens, 10);
        assert_eq!(
            measurement.measurement_method,
            "provider_usage_scaled_estimate"
        );
    }

    #[test]
    fn inference_run_serializes_context_economics_as_camel_case() {
        let run = InferenceRun {
            id: "run-1".into(),
            timestamp: "2026-08-07T12:00:00Z".into(),
            model: "test-model".into(),
            baseline_input_tokens: 100,
            optimized_input_tokens: 60,
            tokens_saved: 40,
            reduction_percent: 40.0,
            actual_input_tokens: Some(62),
            output_tokens: Some(10),
            context_budget_tokens: 80,
            context_estimated_tokens: 55,
            context_units_considered: 12,
            context_units_sent: 8,
            context_units_omitted: 4,
            context_detail_level: "detailed".into(),
            provider_preflight_input_tokens: Some(64),
            cache_read_input_tokens: Some(20),
            cache_write_input_tokens: None,
            latency_ms: 25,
            estimated_cost_usd: Some(0.01),
            memory_count: 3,
            mode: "optimized".into(),
            memory_provider: "local-profile".into(),
            measurement_method: "test".into(),
        };

        let value = serde_json::to_value(run).unwrap();
        assert_eq!(value["contextBudgetTokens"], 80);
        assert_eq!(value["contextUnitsOmitted"], 4);
        assert_eq!(value["providerPreflightInputTokens"], 64);
        assert_eq!(value["cacheReadInputTokens"], 20);
        assert!(value["cacheWriteInputTokens"].is_null());
    }
}
