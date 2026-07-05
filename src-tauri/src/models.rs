use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageSummary {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_cost_usd: f64,
    pub request_count: i64,
    pub unpriced_rows: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourBucket {
    pub hour: String,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRow {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRow {
    pub provider_id: String,
    pub name: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodayDetail {
    pub hours: Vec<HourBucket>,
    pub by_model: Vec<ModelRow>,
    pub by_provider: Vec<ProviderRow>,
}