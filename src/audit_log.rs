//! Local audit/activity log (opt-in). Same format as the JS SDK: sdk/AUDIT_LOG_FORMAT.md.
//! Enable with LICENSEFLOW_AUDIT_LOG=1. Never writes full keys, tokens or passwords.
//! Uses only std (no extra crates); timestamps are UTC.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Default, Clone)]
pub struct AuditEvent {
    pub actor: Option<String>,
    pub action: String,
    pub license: Option<String>,
    pub result: String,
    pub status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub request_id: Option<String>,
    pub detail: Option<String>,
}

pub fn mask_license(key: Option<&str>) -> String {
    let k = match key { Some(k) if !k.is_empty() => k, _ => return "-".into() };
    let chars: Vec<char> = k.chars().collect();
    if chars.len() <= 4 { return "****".into(); }
    let prefix = match k.find('-') {
        Some(i) if (2..=4).contains(&i) && k[..i].chars().all(|c| c.is_ascii_alphabetic()) => &k[..=i],
        _ => "",
    };
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}****-{tail}")
}

/// Redacts lf_live_/lf_test_ keys, Bearer tokens and JWTs; strips newlines.
pub fn scrub_secrets(s: &str) -> String {
    let mut out = Vec::new();
    let tokens: Vec<&str> = s.split(' ').collect();
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        if t == "Bearer" && i + 1 < tokens.len() { out.push("[redacted]".to_string()); i += 2; continue; }
        if let Some(pos) = ["lf_live_", "lf_test_", "eyJ"].iter().filter_map(|p| t.find(p)).min() {
            out.push(format!("{}[redacted]", &t[..pos]));
        } else { out.push(t.to_string()); }
        i += 1;
    }
    out.join(" ").replace(['\r', '\n'], " ")
}

fn civil(secs: u64) -> (i64, u32, u32, u64) {
    let days = (secs / 86400) as i64; let rem = secs % 86400;
    let z = days + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d, rem)
}

pub fn format_audit_line(e: &AuditEvent, now: SystemTime) -> String {
    let dur = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let (y, mo, d, rem) = civil(dur.as_secs());
    let ts = format!("{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", rem / 3600, rem / 60 % 60, rem % 60, dur.subsec_millis());
    let opt = |v: Option<String>| v.unwrap_or_else(|| "-".into());
    let mut parts = vec![ts,
        format!("actor={}", e.actor.clone().unwrap_or_else(|| "app".into())),
        format!("action={}", e.action), format!("license={}", mask_license(e.license.as_deref())),
        format!("result={}", e.result), format!("status={}", opt(e.status.map(|s| s.to_string()))),
        format!("duration_ms={}", opt(e.duration_ms.map(|s| s.to_string()))),
        format!("request_id={}", opt(e.request_id.clone()))];
    if let Some(detail) = &e.detail {
        let dd: String = scrub_secrets(detail).replace('"', "'").chars().take(300).collect();
        parts.push(format!("detail=\"{dd}\""));
    }
    scrub_secrets(&parts.join(" | "))
}

pub struct AuditLogger { pub enabled: bool, pub dir: PathBuf, pub max_bytes: u64, pub retention_days: u64 }

impl Default for AuditLogger {
    fn default() -> Self {
        let v = std::env::var("LICENSEFLOW_AUDIT_LOG").unwrap_or_default().to_lowercase();
        let dir = if cfg!(windows) {
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("LicenseFlow").join("logs")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".licenseflow").join("logs")
        };
        Self { enabled: matches!(v.as_str(), "1" | "true" | "yes"), dir, max_bytes: 10 << 20, retention_days: 14 }
    }
}

impl AuditLogger {
    pub fn write(&self, e: &AuditEvent) {
        if !self.enabled { return; }
        let _ = (|| -> std::io::Result<()> {
            fs::create_dir_all(&self.dir)?;
            let now = SystemTime::now();
            let secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let (y, m, d, _) = civil(secs);
            let path = self.dir.join(format!("licenseflow-audit-{y:04}-{m:02}-{d:02}.txt"));
            if fs::metadata(&path).map(|md| md.len() >= self.max_bytes).unwrap_or(false) {
                fs::rename(&path, self.dir.join(format!("licenseflow-audit-{y:04}-{m:02}-{d:02}-{secs}.txt")))?;
            }
            let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
            writeln!(f, "{}", format_audit_line(e, now))?;
            let cutoff = now - Duration::from_secs(self.retention_days * 86400);
            for entry in fs::read_dir(&self.dir)?.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("licenseflow-audit-") && entry.metadata().and_then(|m| m.modified()).map(|t| t < cutoff).unwrap_or(false) {
                    let _ = fs::remove_file(entry.path());
                }
            }
            Ok(())
        })();
    }
}
