//! Value objects — small, immutable types that carry a business
//! constraint on construction. The type system prevents invalid values
//! from reaching the domain layer.

pub mod actor;
pub mod email;
pub mod language;
pub mod page_status;
pub mod role;
pub mod slug;

pub use actor::Actor;
pub use email::Email;
pub use language::Language;
pub use page_status::PageStatus;
pub use role::Role;
pub use slug::Slug;
