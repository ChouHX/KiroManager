//! 读写 ~/.aws/sso/cache/kiro-auth-token.json 及对应 clientReg 文件.

use anyhow::{anyhow, Context, Result};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::models::Account;

pub fn cache_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".aws").join("sso").join("cache")
}

pub fn token_path() -> PathBuf {
    cache_dir().join("kiro-auth-token.json")
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LocalToken {
    #[serde(rename = "accessToken", default)]
    pub access_token: String,
    #[serde(rename = "refreshToken", default)]
    pub refresh_token: String,
    #[serde(rename = "expiresAt", default)]
    pub expires_at: String,
    #[serde(rename = "authMethod", default)]
    pub auth_method: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub region: String,
    #[serde(rename = "clientIdHash", default)]
    pub client_id_hash: String,
}

pub fn read_local_token() -> Option<LocalToken> {
    let p = token_path();
    if !p.exists() { return None; }
    let s = fs::read_to_string(p).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn clear_local_token() -> Result<()> {
    let p = token_path();
    if p.exists() { fs::remove_file(&p).with_context(|| format!("remove {}", p.display()))?; }
    Ok(())
}

fn parse_client_reg(hash: &str) -> (String, String) {
    let p = cache_dir().join(format!("{}.json", hash));
    let Ok(s) = fs::read_to_string(&p) else { return (String::new(), String::new()); };
    let v: Value = match serde_json::from_str(&s) { Ok(v) => v, Err(_) => return (String::new(), String::new()) };
    (
        v["clientId"].as_str().unwrap_or("").into(),
        v["clientSecret"].as_str().unwrap_or("").into(),
    )
}

fn normalize_expires(e: &str) -> String {
    if e.is_empty() { return String::new(); }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(e, "%Y-%m-%dT%H:%M:%S%.3fZ") {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    e.to_string()
}

fn to_iso_expires(e: &str) -> String {
    if e.is_empty() {
        return (chrono::Utc::now() + Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y/%m/%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S%.3fZ"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(e, fmt) {
            return dt.format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
        }
    }
    (chrono::Utc::now() + Duration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S.000Z").to_string()
}

fn parse_client_secret_expiry(secret: &str) -> String {
    let parts: Vec<&str> = secret.split('.').collect();
    if parts.len() < 2 {
        return (chrono::Utc::now() + Duration::days(90))
            .format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    }
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(parts[1].as_bytes()) else {
        return (chrono::Utc::now() + Duration::days(90))
            .format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    };
    let Ok(v): Result<Value, _> = serde_json::from_slice(&bytes) else {
        return (chrono::Utc::now() + Duration::days(90))
            .format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    };
    let Ok(serialized) = serde_json::from_str::<Value>(
        v["serialized"].as_str().unwrap_or("{}")
    ) else {
        return (chrono::Utc::now() + Duration::days(90))
            .format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    };
    let ts = serialized["expirationTimestamp"].as_i64().unwrap_or(0);
    chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%S.000Z").to_string())
        .unwrap_or_else(|| (chrono::Utc::now() + Duration::days(90))
            .format("%Y-%m-%dT%H:%M:%S.000Z").to_string())
}

/// 从本机 ~/.aws/sso/cache 导入一个账号. 返回 Account (未入库).
pub fn import_from_local() -> Result<Account> {
    let tok = read_local_token().ok_or_else(|| anyhow!("未找到 kiro-auth-token.json"))?;
    let mut a = Account {
        email: String::new(),
        provider: tok.provider.clone(),
        auth_method: tok.auth_method.clone(),
        access_token: tok.access_token.clone(),
        refresh_token: tok.refresh_token.clone(),
        expires_at: normalize_expires(&tok.expires_at),
        client_id_hash: tok.client_id_hash.clone(),
        region: if tok.region.is_empty() { "us-east-1".into() } else { tok.region.clone() },
        ..Default::default()
    };

    if a.auth_method == "IdC" && !a.client_id_hash.is_empty() {
        let (id, sec) = parse_client_reg(&a.client_id_hash);
        a.client_id = id;
        a.client_secret = sec;
    }

    // 先试 JWT 解邮箱
    let (email, sub) = crate::api::decode_jwt_email(&a.access_token);
    a.email = email;
    a.user_id = sub;

    // 查 profileArn 然后查 usage (也带邮箱)
    let fixed = crate::api::fixed_profile_arn(&a.provider).to_string();
    a.profile_arn = if !fixed.is_empty() { fixed }
        else { crate::api::list_profiles(&a.access_token).unwrap_or(None).unwrap_or_default() };

    if !a.profile_arn.is_empty() {
        if let Ok(u) = crate::api::query_usage(&a.access_token, &a.profile_arn, true) {
            if a.email.is_empty() && !u.email.is_empty() { a.email = u.email.clone(); }
            a.usage_limit = u.usage_limit;
            a.current_usage = u.current_usage;
            a.overage_cap = u.overage_cap;
            a.current_overages = u.current_overages;
            a.overage_status = u.overage_status;
            a.overage_charges = u.overage_charges;
            a.subscription = u.subscription;
            a.last_query_time = Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }

    if a.email.is_empty() {
        a.email = format!("{}_{}_local", a.provider, a.auth_method);
    }

    Ok(a)
}

/// 把账号写入 ~/.aws/sso/cache/kiro-auth-token.json + 对应 clientReg
pub fn inject_account(a: &Account) -> Result<()> {
    let dir = cache_dir();
    fs::create_dir_all(&dir).context("create cache dir")?;

    let mut token = serde_json::json!({
        "accessToken": a.access_token,
        "refreshToken": a.refresh_token,
        "expiresAt": to_iso_expires(&a.expires_at),
    });
    match a.auth_method.as_str() {
        "social" => {
            token["authMethod"] = "social".into();
            token["provider"] = a.provider.clone().into();
        }
        "IdC" => {
            token["authMethod"] = "IdC".into();
            token["provider"] = a.provider.clone().into();
            token["region"] = a.region.clone().into();
            token["clientIdHash"] = a.client_id_hash.clone().into();
        }
        other => return Err(anyhow!("不支持的认证方式: {}", other)),
    }
    fs::write(dir.join("kiro-auth-token.json"),
              serde_json::to_vec_pretty(&token)?)?;

    if a.auth_method == "IdC" && !a.client_id.is_empty()
        && !a.client_secret.is_empty() && !a.client_id_hash.is_empty() {
        let reg = serde_json::json!({
            "clientId": a.client_id,
            "clientSecret": a.client_secret,
            "expiresAt": parse_client_secret_expiry(&a.client_secret),
        });
        fs::write(
            dir.join(format!("{}.json", a.client_id_hash)),
            serde_json::to_vec_pretty(&reg)?,
        )?;
    }
    Ok(())
}
