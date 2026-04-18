//! Domain entities — plain data types with identity. No sqlx, serde,
//! or utoipa derives leak in here; presentation DTOs and persistence
//! rows are separate types that map to/from these.

pub mod chunk;
pub mod collection;
pub mod installation;
pub mod page;
pub mod page_version;
pub mod session;
pub mod user;
pub mod version_history;
pub mod workspace;

pub use chunk::{Chunk, NewChunk};
pub use collection::Collection;
pub use installation::Installation;
pub use page::Page;
pub use page_version::PageVersion;
pub use session::Session;
pub use user::User;
pub use version_history::{VersionHistoryEntry, VersionHistorySummary};
pub use workspace::Workspace;
