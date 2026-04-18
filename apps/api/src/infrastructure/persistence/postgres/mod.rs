//! Postgres implementations of the domain repository ports.

pub mod collection_repository;
pub mod export_repository;
pub mod installation_repository;
pub mod mapper;
pub mod page_repository;
pub mod session_repository;
pub mod user_repository;
pub mod version_history_repository;
pub mod workspace_repository;

pub use collection_repository::PostgresCollectionRepository;
pub use export_repository::PostgresExportRepository;
pub use installation_repository::PostgresInstallationRepository;
pub use page_repository::PostgresPageRepository;
pub use session_repository::PostgresSessionRepository;
pub use user_repository::PostgresUserRepository;
pub use version_history_repository::PostgresVersionHistoryRepository;
pub use workspace_repository::PostgresWorkspaceRepository;
