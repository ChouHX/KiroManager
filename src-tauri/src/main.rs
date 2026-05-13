#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod db;
mod local_kiro;
mod models;

use models::Account;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    conn: Mutex<Connection>,
}

#[tauri::command]
fn list_accounts(state: State<AppState>) -> Result<Vec<Account>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::list_accounts(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_local(state: State<AppState>) -> Result<String, String> {
    let account = local_kiro::import_from_local().map_err(|e| e.to_string())?;
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let email = account.email.clone();
    db::upsert(&conn, &account).map_err(|e| e.to_string())?;
    Ok(email)
}

#[tauri::command]
fn import_json(state: State<AppState>, content: String) -> Result<usize, String> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut count = 0;
    for item in &arr {
        if let Ok(a) = parse_json_account(item) {
            if db::upsert(&conn, &a).is_ok() {
                count += 1;
            }
        }
    }
    Ok(count)
}

#[tauri::command]
fn export_json(state: State<AppState>) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let accounts = db::list_accounts(&conn).map_err(|e| e.to_string())?;
    let arr: Vec<serde_json::Value> = accounts
        .iter()
        .map(|a| {
            serde_json::json!({
                "email": a.email,
                "provider": a.provider,
                "authMethod": a.auth_method,
                "accessToken": a.access_token,
                "refreshToken": a.refresh_token,
                "expiresAt": a.expires_at,
                "clientId": a.client_id,
                "clientSecret": a.client_secret,
                "clientIdHash": a.client_id_hash,
                "region": a.region,
                "profileArn": a.profile_arn,
                "userId": a.user_id,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).map_err(|e| e.to_string())
}


#[tauri::command]
fn refresh_accounts(state: State<AppState>, ids: Vec<i64>) -> Result<Vec<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let accounts = db::list_accounts(&conn).map_err(|e| e.to_string())?;
    let mut logs = Vec::new();

    for &id in &ids {
        let Some(account) = accounts.iter().find(|a| a.id == id) else {
            continue;
        };
        match api::do_refresh(account) {
            Ok((at, rt, exp)) => {
                let _ = db::update_token(&conn, id, &at, &rt, &exp);
                let profile = if !account.profile_arn.is_empty() {
                    account.profile_arn.clone()
                } else {
                    api::fixed_profile_arn(&account.provider).to_string()
                };
                if !profile.is_empty() {
                    if let Ok(u) = api::query_usage(&at, &profile, false) {
                        let _ = db::update_usage(
                            &conn, id, u.usage_limit, u.current_usage,
                            u.overage_cap, u.current_overages,
                            &u.overage_status, u.overage_charges, &u.subscription,
                        );
                    }
                }
                logs.push(format!("✓ {}", account.email));
            }
            Err(e) => {
                logs.push(format!("✗ {}: {}", account.email, e));
            }
        }
    }
    Ok(logs)
}

#[tauri::command]
fn health_check(state: State<AppState>) -> Result<Vec<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let accounts = db::list_accounts(&conn).map_err(|e| e.to_string())?;
    let mut logs = Vec::new();
    let (mut valid, mut refreshed, mut failed) = (0, 0, 0);

    for account in &accounts {
        if !api::is_expired(&account.expires_at) {
            valid += 1;
        } else {
            match api::do_refresh(account) {
                Ok((at, rt, exp)) => {
                    let _ = db::update_token(&conn, account.id, &at, &rt, &exp);
                    refreshed += 1;
                }
                Err(_) => failed += 1,
            }
        }
    }
    logs.push(format!("有效 {valid}, 已刷新 {refreshed}, 无效 {failed}"));
    Ok(logs)
}

#[tauri::command]
fn refresh_all(state: State<AppState>) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let accounts = db::list_accounts(&conn).map_err(|e| e.to_string())?;
    let mut ok_n = 0;
    let total = accounts.len();
    for account in &accounts {
        if account.refresh_token.is_empty() {
            continue;
        }
        match api::do_refresh(account) {
            Ok((at, rt, exp)) => {
                let _ = db::update_token(&conn, account.id, &at, &rt, &exp);
                let profile = if !account.profile_arn.is_empty() {
                    account.profile_arn.clone()
                } else {
                    api::fixed_profile_arn(&account.provider).to_string()
                };
                if !profile.is_empty() {
                    if let Ok(u) = api::query_usage(&at, &profile, false) {
                        let _ = db::update_usage(
                            &conn, account.id, u.usage_limit, u.current_usage,
                            u.overage_cap, u.current_overages,
                            &u.overage_status, u.overage_charges, &u.subscription,
                        );
                    }
                }
                ok_n += 1;
            }
            Err(_) => {}
        }
    }
    Ok(format!("自动刷新: {ok_n}/{total}"))
}

#[tauri::command]
fn delete_accounts(state: State<AppState>, ids: Vec<i64>) -> Result<usize, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut count = 0;
    for &id in &ids {
        if db::delete(&conn, id).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

#[tauri::command]
fn inject_to_local(state: State<AppState>, id: i64) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let accounts = db::list_accounts(&conn).map_err(|e| e.to_string())?;
    let account = accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or("账号不存在")?;
    local_kiro::inject_account(account).map_err(|e| e.to_string())?;
    Ok(account.email.clone())
}

#[tauri::command]
fn get_local_token() -> Result<Option<LocalTokenInfo>, String> {
    Ok(local_kiro::read_local_token().map(|t| LocalTokenInfo {
        auth_method: t.auth_method,
        provider: t.provider,
        region: t.region,
        expires_at: t.expires_at.clone(),
        client_id_hash: t.client_id_hash,
        access_token_preview: truncate_str(&t.access_token, 60),
        refresh_token_preview: truncate_str(&t.refresh_token, 60),
        is_expired: api::is_expired(&t.expires_at),
    }))
}

#[tauri::command]
fn refresh_local_token() -> Result<String, String> {
    let tok = local_kiro::read_local_token().ok_or("本地无 Token")?;
    let mut account = Account {
        auth_method: tok.auth_method.clone(),
        provider: tok.provider.clone(),
        access_token: tok.access_token.clone(),
        refresh_token: tok.refresh_token.clone(),
        region: if tok.region.is_empty() { "us-east-1".into() } else { tok.region.clone() },
        client_id_hash: tok.client_id_hash.clone(),
        ..Default::default()
    };
    if account.auth_method == "IdC" && !account.client_id_hash.is_empty() {
        let reg_path = local_kiro::cache_dir().join(format!("{}.json", account.client_id_hash));
        if let Ok(s) = std::fs::read_to_string(&reg_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                account.client_id = v["clientId"].as_str().unwrap_or("").to_string();
                account.client_secret = v["clientSecret"].as_str().unwrap_or("").to_string();
            }
        } else {
            return Err("缺少 clientRegistration 文件".into());
        }
    }
    let (at, rt, exp) = api::do_refresh(&account).map_err(|e| e.to_string())?;
    account.access_token = at;
    account.refresh_token = rt;
    account.expires_at = exp;
    local_kiro::inject_account(&account).map_err(|e| e.to_string())?;
    Ok("本地 Token 已刷新".into())
}

#[tauri::command]
fn clear_local_token() -> Result<(), String> {
    local_kiro::clear_local_token().map_err(|e| e.to_string())
}

#[tauri::command]
fn enable_overage_for(state: State<AppState>, id: i64) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let accounts = db::list_accounts(&conn).map_err(|e| e.to_string())?;
    let account = accounts.iter().find(|a| a.id == id).ok_or("账号不存在")?;

    // 跳过已开启的
    if account.overage_status.to_uppercase() == "ENABLED" {
        return Ok(format!("{} 超额已开启(跳过)", account.email));
    }

    // 跳过 Free 账号
    let sub_lower = account.subscription.to_lowercase();
    if !sub_lower.is_empty() && sub_lower.contains("free") && !sub_lower.contains("pro") && !sub_lower.contains("power") {
        return Ok(format!("{} Free订阅不支持超额(跳过)", account.email));
    }

    // 确保 token 有效
    let access_token = if api::is_expired(&account.expires_at) {
        let (at, rt, exp) = api::do_refresh(account).map_err(|e| format!("{}: {}", account.email, e))?;
        let _ = db::update_token(&conn, id, &at, &rt, &exp);
        at
    } else {
        account.access_token.clone()
    };

    // 获取 profileArn: 数据库 > 固定值 > API 查询
    let profile = if !account.profile_arn.is_empty() {
        account.profile_arn.clone()
    } else {
        let fixed = api::fixed_profile_arn(&account.provider).to_string();
        if !fixed.is_empty() {
            fixed
        } else {
            api::list_profiles(&access_token)
                .unwrap_or(None)
                .unwrap_or_default()
        }
    };
    if profile.is_empty() {
        return Err(format!("{}: 无法获取 profileArn", account.email));
    }

    api::enable_overage(&access_token, &profile)
        .map_err(|e| format!("{}: {}", account.email, e))?;

    // 更新数据库中的 overage_status
    let _ = db::update_usage(
        &conn, id,
        account.usage_limit, account.current_usage,
        account.overage_cap, account.current_overages,
        "ENABLED", account.overage_charges, &account.subscription,
    );

    Ok(format!("{} 超额已开启", account.email))
}

#[derive(serde::Serialize)]
struct LocalTokenInfo {
    auth_method: String,
    provider: String,
    region: String,
    expires_at: String,
    client_id_hash: String,
    access_token_preview: String,
    refresh_token_preview: String,
    is_expired: bool,
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}

fn parse_json_account(v: &serde_json::Value) -> anyhow::Result<Account> {
    let s = |key: &str| v[key].as_str().unwrap_or("").to_string();
    Ok(Account {
        email: s("email"),
        provider: s("provider"),
        auth_method: s("authMethod"),
        access_token: s("accessToken"),
        refresh_token: s("refreshToken"),
        expires_at: s("expiresAt"),
        client_id: s("clientId"),
        client_secret: s("clientSecret"),
        client_id_hash: s("clientIdHash"),
        region: { let r = s("region"); if r.is_empty() { "us-east-1".into() } else { r } },
        profile_arn: s("profileArn"),
        user_id: s("userId"),
        ..Default::default()
    })
}

fn main() {
    let conn = db::open().expect("打开数据库失败");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState { conn: Mutex::new(conn) })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            import_local,
            import_json,
            export_json,
            refresh_accounts,
            refresh_all,
            health_check,
            delete_accounts,
            inject_to_local,
            enable_overage_for,
            get_local_token,
            refresh_local_token,
            clear_local_token,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 失败");
}
