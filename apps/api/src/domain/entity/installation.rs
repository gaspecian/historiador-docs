use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Installation {
    pub setup_complete: bool,
    pub completed_at: Option<DateTime<Utc>>,
}
