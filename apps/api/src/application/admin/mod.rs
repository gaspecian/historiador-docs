//! Admin use cases — user management, workspace config, analytics.

pub mod deactivate_user;
pub mod get_analytics;
pub mod get_workspace;
pub mod invite_user;
pub mod list_users;
pub mod log_mcp_query;
pub mod regenerate_token;
pub mod reindex_workspace;
pub mod update_llm_config;

pub use deactivate_user::DeactivateUserUseCase;
pub use get_analytics::GetMcpAnalyticsUseCase;
pub use get_workspace::GetWorkspaceUseCase;
pub use invite_user::{InviteUserCommand, InviteUserResult, InviteUserUseCase};
pub use list_users::ListUsersUseCase;
pub use log_mcp_query::{LogMcpQueryCommand, LogMcpQueryUseCase};
pub use regenerate_token::RegenerateTokenUseCase;
pub use reindex_workspace::{ReindexPlan, ReindexWorkspaceUseCase};
pub use update_llm_config::{
    UpdateLlmConfigCommand, UpdateLlmConfigResult, UpdateLlmConfigUseCase,
};
