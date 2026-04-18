//! Clock port — inject testable `now()` into use cases. Default
//! production impl wraps `chrono::Utc::now()`.

use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
