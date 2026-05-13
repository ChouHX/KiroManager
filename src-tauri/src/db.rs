use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

use crate::models::Account;

pub fn db_path() -> PathBuf {
    // 和 Python 版保持一致: exe 所在目录
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    exe.parent().unwrap_or(std::path::Path::new(".")).join("kiro_accounts.db")
}

pub fn open() -> Result<Connection> {
    let p = db_path();
    let conn = Connection::open(&p)
        .with_context(|| format!("open {}", p.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT, password TEXT DEFAULT '',
            provider TEXT, authMethod TEXT,
            accessToken TEXT, refreshToken TEXT, expiresAt TEXT,
            clientId TEXT, clientSecret TEXT, clientIdHash TEXT,
            region TEXT DEFAULT 'us-east-1',
            profileArn TEXT, userId TEXT,
            usageLimit INTEGER DEFAULT 0,
            currentUsage INTEGER DEFAULT 0,
            overageCap INTEGER DEFAULT 0,
            currentOverages INTEGER DEFAULT 0,
            overageStatus TEXT,
            overageCharges REAL DEFAULT 0.0,
            subscription TEXT DEFAULT '',
            lastQueryTime TEXT,
            createdAt TEXT DEFAULT (datetime('now','localtime')),
            updatedAt TEXT DEFAULT (datetime('now','localtime'))
        );",
    )?;
    Ok(conn)
}

pub fn list_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, email, provider, authMethod, accessToken, refreshToken,
                expiresAt, clientId, clientSecret, clientIdHash, region,
                profileArn, userId, usageLimit, currentUsage, overageCap,
                currentOverages, overageStatus, overageCharges,
                IFNULL(subscription,''), lastQueryTime
         FROM accounts ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Account {
            id: r.get(0)?,
            email: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            provider: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            auth_method: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            access_token: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            refresh_token: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            expires_at: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
            client_id: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
            client_secret: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
            client_id_hash: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
            region: r.get::<_, Option<String>>(10)?.unwrap_or_else(|| "us-east-1".into()),
            profile_arn: r.get::<_, Option<String>>(11)?.unwrap_or_default(),
            user_id: r.get::<_, Option<String>>(12)?.unwrap_or_default(),
            usage_limit: r.get::<_, Option<i64>>(13)?.unwrap_or(0),
            current_usage: r.get::<_, Option<i64>>(14)?.unwrap_or(0),
            overage_cap: r.get::<_, Option<i64>>(15)?.unwrap_or(0),
            current_overages: r.get::<_, Option<i64>>(16)?.unwrap_or(0),
            overage_status: r.get::<_, Option<String>>(17)?.unwrap_or_default(),
            overage_charges: r.get::<_, Option<f64>>(18)?.unwrap_or(0.0),
            subscription: r.get::<_, Option<String>>(19)?.unwrap_or_default(),
            last_query_time: r.get::<_, Option<String>>(20)?,
        })
    })?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

pub fn get_account(conn: &Connection, id: i64) -> Result<Option<Account>> {
    let list = list_accounts(conn)?;
    Ok(list.into_iter().find(|a| a.id == id))
}

pub fn upsert(conn: &Connection, a: &Account) -> Result<i64> {
    // 先按 userId, 再按 email 匹配
    let existing: Option<i64> = if !a.user_id.is_empty() {
        conn.query_row(
            "SELECT id FROM accounts WHERE userId=?1", [&a.user_id], |r| r.get(0),
        ).optional()?
    } else { None };
    let existing = if existing.is_none() && !a.email.is_empty() {
        conn.query_row(
            "SELECT id FROM accounts WHERE email=?1", [&a.email], |r| r.get(0),
        ).optional()?
    } else { existing };

    if let Some(id) = existing {
        conn.execute(
            "UPDATE accounts SET
                email=?1, provider=?2, authMethod=?3,
                accessToken=?4, refreshToken=?5, expiresAt=?6,
                clientId=?7, clientSecret=?8, clientIdHash=?9,
                region=?10, profileArn=?11, userId=?12,
                usageLimit=?13, currentUsage=?14, overageCap=?15,
                currentOverages=?16, overageStatus=?17, overageCharges=?18,
                subscription=?19, lastQueryTime=?20,
                updatedAt=datetime('now','localtime')
             WHERE id=?21",
            params![
                a.email, a.provider, a.auth_method,
                a.access_token, a.refresh_token, a.expires_at,
                a.client_id, a.client_secret, a.client_id_hash,
                a.region, a.profile_arn, a.user_id,
                a.usage_limit, a.current_usage, a.overage_cap,
                a.current_overages, a.overage_status, a.overage_charges,
                a.subscription, a.last_query_time, id,
            ],
        )?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO accounts (
                email, provider, authMethod, accessToken, refreshToken,
                expiresAt, clientId, clientSecret, clientIdHash, region,
                profileArn, userId, usageLimit, currentUsage, overageCap,
                currentOverages, overageStatus, overageCharges, subscription,
                lastQueryTime
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
            params![
                a.email, a.provider, a.auth_method,
                a.access_token, a.refresh_token, a.expires_at,
                a.client_id, a.client_secret, a.client_id_hash,
                a.region, a.profile_arn, a.user_id,
                a.usage_limit, a.current_usage, a.overage_cap,
                a.current_overages, a.overage_status, a.overage_charges,
                a.subscription, a.last_query_time,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn update_token(conn: &Connection, id: i64, access: &str, refresh: &str, expires: &str) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET accessToken=?1, refreshToken=?2, expiresAt=?3,
            updatedAt=datetime('now','localtime') WHERE id=?4",
        params![access, refresh, expires, id],
    )?;
    Ok(())
}

pub fn update_usage(
    conn: &Connection, id: i64,
    usage_limit: i64, current_usage: i64,
    overage_cap: i64, current_overages: i64,
    overage_status: &str, overage_charges: f64,
    subscription: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET
            usageLimit=?1, currentUsage=?2, overageCap=?3, currentOverages=?4,
            overageStatus=?5, overageCharges=?6, subscription=?7,
            lastQueryTime=datetime('now','localtime'),
            updatedAt=datetime('now','localtime')
         WHERE id=?8",
        params![usage_limit, current_usage, overage_cap, current_overages,
                overage_status, overage_charges, subscription, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM accounts WHERE id=?1", [id])?;
    Ok(())
}
