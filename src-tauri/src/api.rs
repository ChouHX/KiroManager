//! Kiro / AWS API 客户端. 对齐 Python 版 api.py.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Duration;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::Account;

pub const KIRO_AUTH: &str = "https://prod.us-east-1.auth.desktop.kiro.dev";
pub const CODEWHISPERER: &str = "https://q.us-east-1.amazonaws.com";

pub fn fixed_profile_arn(provider: &str) -> &'static str {
    match provider {
        "BuilderId" => "arn:aws:codewhisperer:us-east-1:638616132270:profile/AAAACCCCXXXX",
        "Github" | "Google" => "arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK",
        _ => "",
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(20))
        .build()
}

// ── Token 刷新 ─────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
struct RefreshResp {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(default, rename = "expiresIn")]
    expires_in: i64,
    #[serde(default, rename = "idToken")]
    id_token: Option<String>,
}

pub fn refresh_social(refresh_token: &str) -> Result<(String, String, String)> {
    let url = format!("{}/refreshToken", KIRO_AUTH);
    let resp: RefreshResp = agent().post(&url).send_json(json!({
        "refreshToken": refresh_token
    }))?.into_json().context("parse refresh response")?;
    Ok((resp.access_token, resp.refresh_token, expires_in_to_ts(resp.expires_in)))
}

pub fn refresh_idc(client_id: &str, client_secret: &str, refresh_token: &str, region: &str) -> Result<(String, String, String, Option<String>)> {
    let url = format!("https://oidc.{}.amazonaws.com/token", region);
    let resp: RefreshResp = agent().post(&url).send_json(json!({
        "clientId": client_id,
        "clientSecret": client_secret,
        "refreshToken": refresh_token,
        "grantType": "refresh_token"
    }))?.into_json().context("parse IdC refresh response")?;
    Ok((resp.access_token, resp.refresh_token, expires_in_to_ts(resp.expires_in), resp.id_token))
}

fn expires_in_to_ts(secs: i64) -> String {
    let secs = if secs <= 0 { 3600 } else { secs };
    (chrono::Local::now() + Duration::seconds(secs))
        .format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 刷新. 成功后返回 (access, refresh, expires_at).
pub fn do_refresh(a: &Account) -> Result<(String, String, String)> {
    match a.auth_method.as_str() {
        "social" => refresh_social(&a.refresh_token),
        "IdC" => {
            if a.client_id.is_empty() || a.client_secret.is_empty() {
                return Err(anyhow!("缺少 clientId/clientSecret"));
            }
            let region = if a.region.is_empty() { "us-east-1" } else { &a.region };
            let (at, rt, exp, _) = refresh_idc(&a.client_id, &a.client_secret, &a.refresh_token, region)?;
            Ok((at, rt, exp))
        }
        other => Err(anyhow!("未知认证方式: {}", other)),
    }
}

// ── 是否过期 ──────────────────────────────────────────────────────────
pub fn is_expired(expires_at: &str) -> bool {
    if expires_at.is_empty() { return true; }
    let now = chrono::Local::now().naive_local() + Duration::minutes(5);
    for fmt in ["%Y-%m-%dT%H:%M:%S%.3fZ", "%Y-%m-%d %H:%M:%S", "%Y/%m/%d %H:%M:%S"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(expires_at, fmt) {
            return dt < now;
        }
    }
    true
}

// ── Usage ─────────────────────────────────────────────────────────────
pub fn list_profiles(access_token: &str) -> Result<Option<String>> {
    let url = format!("{}/ListAvailableProfiles", CODEWHISPERER);
    let v: Value = agent().post(&url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .send_json(json!({}))?.into_json()?;
    Ok(v["profiles"].as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["arn"].as_str())
        .map(|s| s.to_string()))
}

#[derive(Debug, Clone, Default)]
pub struct UsageInfo {
    pub usage_limit: i64,
    pub current_usage: i64,
    pub overage_cap: i64,
    pub current_overages: i64,
    pub overage_status: String,
    pub overage_charges: f64,
    pub subscription: String,
    pub email: String,
}

pub fn query_usage(access_token: &str, profile_arn: &str, email_required: bool) -> Result<UsageInfo> {
    let q = if email_required { "&isEmailRequired=true" } else { "" };
    let url = format!(
        "{}/getUsageLimits?profileArn={}{}",
        CODEWHISPERER,
        url::form_urlencoded::byte_serialize(profile_arn.as_bytes()).collect::<String>(),
        q,
    );
    let v: Value = agent().get(&url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .call()?.into_json()?;
    Ok(parse_usage(&v))
}

fn parse_usage(v: &Value) -> UsageInfo {
    let b = v["usageBreakdownList"].as_array()
        .and_then(|a| a.first()).cloned().unwrap_or(Value::Null);
    let sub = v["subscriptionInfo"].clone();

    let as_i64 = |k1: &str, k2: &str| -> i64 {
        b.get(k1).and_then(|x| x.as_i64())
            .or_else(|| b.get(k2).and_then(|x| x.as_i64()))
            .unwrap_or(0)
    };

    UsageInfo {
        usage_limit: as_i64("usageLimit", "usageLimitWithPrecision"),
        current_usage: as_i64("currentUsage", "currentUsageWithPrecision"),
        overage_cap: as_i64("overageCap", "overageCapWithPrecision"),
        current_overages: as_i64("currentOverages", "currentOveragesWithPrecision"),
        overage_status: v["overageConfiguration"]["overageStatus"].as_str().unwrap_or("").into(),
        overage_charges: b.get("overageCharges").and_then(|x| x.as_f64()).unwrap_or(0.0),
        subscription: sub.get("subscriptionTitle").and_then(|x| x.as_str())
            .or_else(|| sub.get("type").and_then(|x| x.as_str()))
            .unwrap_or("").to_string(),
        email: v["userInfo"]["email"].as_str().unwrap_or("").to_string(),
    }
}

pub fn enable_overage(access_token: &str, profile_arn: &str) -> Result<()> {
    let url = format!("{}/setUserPreference", CODEWHISPERER);
    agent().post(&url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .send_json(json!({
            "profileArn": profile_arn,
            "overageConfiguration": { "overageStatus": "ENABLED" }
        }))?;
    Ok(())
}

// ── JWT 解析 ─────────────────────────────────────────────────────────
pub fn decode_jwt_email(token: &str) -> (String, String) {
    if token.is_empty() { return (String::new(), String::new()); }
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 { return (String::new(), String::new()); }
    let pad = (4 - parts[1].len() % 4) % 4;
    let mut s = parts[1].to_string();
    s.push_str(&"=".repeat(pad));
    let bytes = match URL_SAFE_NO_PAD.decode(&parts[1].as_bytes()) {
        Ok(b) => b,
        Err(_) => return (String::new(), String::new()),
    };
    let v: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return (String::new(), String::new()),
    };
    let email = v["email"].as_str()
        .or_else(|| v["preferred_username"].as_str())
        .unwrap_or("").to_string();
    let sub = v["sub"].as_str().unwrap_or("").to_string();
    (email, sub)
}
