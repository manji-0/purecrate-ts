use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub user_id: i64,
    pub display_name: String,
    #[serde(rename = "e-mail")]
    pub email: Option<String>,
    #[serde(rename = "quote\"and\\slash\tand tab")]
    pub odd: bool,
    pub created_at_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub struct Limits {
    pub max_items: u32,
    pub per_page: u8,
    #[serde(rename = "ok?")]
    pub ok: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    OnHold,
    #[serde(rename = "gone!")]
    Deleted,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    Opened {
        at_ms: u64,
    },
    #[serde(rename_all = "PascalCase")]
    Renamed {
        old_name: String,
        new_name: String,
    },
    Moved(i32, i32),
    Closed(Status),
    Pinged,
}

#[derive(Serialize, Deserialize)]
pub struct Envelope {
    pub account: Account,
    pub limits: Limits,
    pub events: Vec<Event>,
}
