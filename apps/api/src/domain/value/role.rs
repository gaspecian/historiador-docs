//! User role with an ordering for permission checks.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Admin,
    Author,
    Viewer,
}

impl Role {
    /// A user whose role rank >= required rank is authorized.
    pub fn rank(&self) -> u8 {
        match self {
            Role::Viewer => 0,
            Role::Author => 1,
            Role::Admin => 2,
        }
    }

    pub fn at_least(&self, required: Role) -> bool {
        self.rank() >= required.rank()
    }
}
