use serde::{Deserialize, Serialize};

/// 账号行, 跟 Python 版 sqlite schema 对齐.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: String,
    pub auth_method: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: String,
    pub client_id: String,
    pub client_secret: String,
    pub client_id_hash: String,
    pub region: String,
    pub profile_arn: String,
    pub user_id: String,
    pub usage_limit: i64,
    pub current_usage: i64,
    pub overage_cap: i64,
    pub current_overages: i64,
    pub overage_status: String,
    pub overage_charges: f64,
    pub subscription: String,
    pub last_query_time: Option<String>,
}

impl Account {
    pub fn usage_str(&self) -> String {
        format!("{}/{}", self.current_usage, self.usage_limit)
    }

    pub fn subscription_display(&self) -> String {
        format_subscription(&self.subscription)
    }
}

pub fn format_subscription(raw: &str) -> String {
    if raw.is_empty() {
        return "-".into();
    }
    let upper = raw.to_uppercase().replace(' ', "_");
    let table = [
        ("KIRO_PRO_PLUS", "Pro+"),
        ("KIRO_PRO", "Pro"),
        ("KIRO_FREE", "Free"),
        ("KIRO_POWER", "Power"),
        ("Q_DEVELOPER_STANDALONE_PRO_PLUS", "Pro+"),
        ("Q_DEVELOPER_STANDALONE_PRO", "Pro"),
        ("Q_DEVELOPER_STANDALONE_FREE", "Free"),
        ("Q_DEVELOPER_STANDALONE_POWER", "Power"),
        ("Q_DEVELOPER_STANDALONE", "Free"),
    ];
    for (k, v) in table {
        if upper.contains(k) {
            return v.into();
        }
    }
    if upper.contains("PRO") { "Pro".into() }
    else if upper.contains("FREE") { "Free".into() }
    else { raw.to_string() }
}
