//! ID generator port — injectable UUID production for deterministic
//! tests. Default production impl emits v4/v7 UUIDs.

use uuid::Uuid;

pub trait IdGenerator: Send + Sync {
    fn new_id(&self) -> Uuid;
}
