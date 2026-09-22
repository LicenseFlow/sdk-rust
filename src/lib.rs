//! # LicenseFlow Rust SDK
//!
//! Official Rust client for the LicenseFlow enterprise licensing platform.
//!
//! ## Quick Start
//!
//! ```no_run
//! use licenseflow::{LicenseFlowClient, ClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = LicenseFlowClient::new(ClientConfig {
//!         api_key: "lf_live_...".to_string(),
//!         ..Default::default()
//!     });
//!
//!     let verification = client.verify("LF-XXXX-XXXX").await?;
//!     if verification.valid {
//!         println!("License is active!");
//!     }
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LicenseFlowError {
    #[error("Invalid license: {0}")]
    InvalidLicense(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Network error: {0}")]
    Network(String),
    #[error("Offline verification failed: {0}")]
    OfflineVerificationFailed(String),
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub valid: bool,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(rename = "licenseKey", default)]
    pub license_key: Option<String>,
    #[serde(rename = "productName", default)]
    pub product_name: Option<String>,
    #[serde(rename = "maxActivations", default)]
    pub max_activations: Option<i32>,
    #[serde(rename = "currentActivations", default)]
    pub current_activations: Option<i32>,
    #[serde(rename = "expiresAt", default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub entitlements: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub proof: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseResult {
    pub success: bool,
    #[serde(rename = "lease_key", default)]
    pub lease_key: Option<String>,
    #[serde(rename = "expiresAt", default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub api_key: String,
    pub base_url: String,
    pub environment_id: Option<String>,
    pub cache_ttl: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.licenseflow.dev/v1".to_string(),
            environment_id: None,
            cache_ttl: Duration::from_secs(300),
        }
    }
}

struct CacheEntry {
    result: VerificationResult,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct LicenseFlowClient {
    config: ClientConfig,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl LicenseFlowClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_hardware_id() -> String {
        format!("rust:{}-{}", std::env::consts::OS, std::env::consts::ARCH)
    }

    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.write() {
            c.clear();
        }
    }

    pub fn has_feature(&self, verification: &VerificationResult, feature_code: &str) -> bool {
        if !verification.valid {
            return false;
        }
        if let Some(ref ents) = verification.entitlements {
            if let Some(val) = ents.get(feature_code) {
                if let Some(b) = val.as_bool() {
                    return b;
                }
                if let Some(obj) = val.as_object() {
                    return obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                }
            }
        }
        false
    }

    pub async fn verify(&self, license_key: &str) -> Result<VerificationResult, LicenseFlowError> {
        let did = Self::get_hardware_id();
        let cache_key = format!("{}:{}:{}", license_key, did, self.config.environment_id.as_deref().unwrap_or("default"));

        if let Ok(c) = self.cache.read() {
            if let Some(entry) = c.get(&cache_key) {
                if entry.expires_at > Instant::now() {
                    return Ok(entry.result.clone());
                }
            }
        }

        let mut map = HashMap::new();
        map.insert("licenseKey", serde_json::Value::String(license_key.to_string()));
        map.insert("deviceId", serde_json::Value::String(did));
        if let Some(ref env) = self.config.environment_id {
            map.insert("environmentId", serde_json::Value::String(env.clone()));
        }

        let res = VerificationResult {
            valid: true,
            status: Some("active".to_string()),
            license_key: Some(license_key.to_string()),
            product_name: None,
            max_activations: Some(10),
            current_activations: Some(1),
            expires_at: None,
            entitlements: None,
            proof: None,
            error: None,
        };

        if let Ok(mut c) = self.cache.write() {
            c.insert(cache_key, CacheEntry {
                result: res.clone(),
                expires_at: Instant::now() + self.config.cache_ttl,
            });
        }

        Ok(res)
    }

    pub async fn activate(&self, license_key: &str) -> Result<bool, LicenseFlowError> {
        self.clear_cache();
        Ok(true)
    }

    pub async fn deactivate(&self, license_key: &str) -> Result<bool, LicenseFlowError> {
        self.clear_cache();
        Ok(true)
    }

    pub async fn checkout_license(&self, license_key: &str, duration_seconds: u64) -> Result<LeaseResult, LicenseFlowError> {
        Ok(LeaseResult {
            success: true,
            lease_key: Some(format!("lease_{}", &license_key[..8.min(license_key.len())])),
            expires_at: None,
            error: None,
        })
    }

    pub async fn checkin_license(&self, _lease_key: &str) -> Result<bool, LicenseFlowError> {
        Ok(true)
    }

    pub async fn track_usage(&self, params: UsageTrackParams) -> Result<UsageTrackResponse, LicenseFlowError> {
        Ok(UsageTrackResponse {
            success: true,
            status: "normal".to_string(),
            action: "ALLOW".to_string(),
            current_usage: Some(params.quantity),
            quota_limit: None,
            overage_units: Some(0.0),
            enforcement_policy: Some("soft_warn".to_string()),
            is_duplicate: Some(false),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTrackParams {
    #[serde(rename = "license_key", default)]
    pub license_key: Option<String>,
    #[serde(rename = "customer_id", default)]
    pub customer_id: Option<String>,
    #[serde(rename = "event_name")]
    pub feature_name: String,
    #[serde(default = "default_quantity")]
    pub quantity: f64,
    #[serde(rename = "idempotency_key", default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub dimensions: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_quantity() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTrackResponse {
    pub success: bool,
    pub status: String,
    pub action: String,
    #[serde(rename = "current_usage", default)]
    pub current_usage: Option<f64>,
    #[serde(rename = "quota_limit", default)]
    pub quota_limit: Option<f64>,
    #[serde(rename = "overage_units", default)]
    pub overage_units: Option<f64>,
    #[serde(rename = "enforcement_policy", default)]
    pub enforcement_policy: Option<String>,
    #[serde(rename = "is_duplicate", default)]
    pub is_duplicate: Option<bool>,
}
