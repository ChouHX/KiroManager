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

