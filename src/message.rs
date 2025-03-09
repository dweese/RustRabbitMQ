
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]

pub struct RRMessage {
    pub content: String,
    pub message_type: Option<String>,
}
