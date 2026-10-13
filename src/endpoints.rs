//! Canonical REST paths (SDK v2.3+). The api.licenseflow.dev gateway serves
//! both these and the legacy `/functions/v1/<name>` paths indefinitely.

/// (legacy function name, canonical path)
pub const CANONICAL_ENDPOINTS: &[(&str, &str)] = &[
    ("verify-license", "/v1/licenses/verify"),
    ("activate-license", "/v1/licenses/activate"),
    ("deactivate-license", "/v1/licenses/deactivate"),
    ("checkout-license", "/v1/licenses/checkout"),
    ("lease-status", "/v1/licenses/lease-status"),
    ("generate-offline-license", "/v1/licenses/offline"),
    ("record-usage", "/v1/usage/record"),
    ("resolve-entitlements", "/v1/entitlements/resolve"),
    ("check-entitlement", "/v1/entitlements/check"),
    ("authorize", "/v1/access/authorize"),
];

/// Canonical path for a legacy function name, if one exists.
pub fn canonical_path(function_name: &str) -> Option<&'static str> {
    CANONICAL_ENDPOINTS.iter().find(|(n, _)| *n == function_name).map(|(_, p)| *p)
}
