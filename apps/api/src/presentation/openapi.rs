//! OpenAPI registry. Every `#[utoipa::path]`-annotated handler and
//! every `ToSchema`-deriving DTO must appear here to be included in
//! the generated `openapi.yaml` contract.

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::infrastructure::llm::probe as llm_probe;
use crate::presentation::handler::admin::{
    analytics as admin_analytics, users as admin_users, workspace as admin_workspace,
};
use crate::presentation::handler::{auth, collections, editor, export, health, pages, setup};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::handler,
        setup::init,
        setup::probe,
        setup::ollama_models,
        auth::login,
        auth::refresh,
        auth::logout,
        auth::activate,
        admin_users::invite,
        admin_users::list_users,
        admin_users::deactivate_user,
        admin_workspace::get_workspace,
        admin_workspace::regenerate_token,
        admin_workspace::update_llm_config,
        admin_workspace::reindex,
        collections::create_collection,
        collections::list_collections,
        collections::update_collection,
        collections::delete_collection,
        pages::list_pages,
        pages::search_pages,
        pages::create_page,
        pages::get_page,
        pages::get_page_versions,
        pages::update_page,
        pages::publish_page,
        pages::draft_page,
        pages::list_version_history,
        pages::get_version_history_item,
        pages::restore_version,
        admin_analytics::get_mcp_analytics,
        editor::draft,
        editor::iterate,
        export::export_workspace,
        export::export_page,
    ),
    components(schemas(
        health::HealthResponse,
        setup::SetupRequest,
        setup::SetupResponse,
        setup::ProbeRequest,
        setup::ProbeResponse,
        setup::OllamaModelsRequest,
        setup::OllamaModelsResponse,
        setup::OllamaModelEntry,
        llm_probe::LlmProvider,
        auth::LoginRequest,
        auth::TokenResponse,
        auth::RefreshRequest,
        auth::LogoutRequest,
        auth::ActivateRequest,
        admin_users::InviteRequest,
        admin_users::InviteResponse,
        admin_users::UserResponse,
        admin_workspace::WorkspaceResponse,
        admin_workspace::RegenerateTokenResponse,
        admin_workspace::LlmPatchRequest,
        admin_workspace::LlmPatchResponse,
        admin_workspace::ReindexResponse,
        collections::CreateCollectionRequest,
        collections::UpdateCollectionRequest,
        collections::CollectionResponse,
        pages::CreatePageRequest,
        pages::UpdatePageRequest,
        pages::PageResponse,
        pages::PageVersionResponse,
        pages::PageVersionsResponse,
        pages::PublishResponse,
        historiador_db::postgres::pages::PageStatus,
        historiador_db::postgres::collections::Collection,
        pages::VersionHistoryListResponse,
        pages::VersionHistorySummary,
        pages::VersionHistoryDetailResponse,
        admin_analytics::McpAnalyticsResponse,
        admin_analytics::DayCountDto,
        admin_analytics::QueryFrequencyDto,
        admin_analytics::ZeroResultSummaryDto,
        admin_analytics::ZeroResultQueryDto,
        editor::DraftRequest,
        editor::IterateRequest,
    )),
    modifiers(&BearerAuth),
    info(
        title = "Historiador Doc API",
        version = "0.1.0",
        description = "REST API for Historiador Doc — self-hosted documentation with a built-in MCP server."
    ),
    tags(
        (name = "system", description = "Health and system metadata"),
        (name = "setup",  description = "First-run installation wizard"),
        (name = "auth",   description = "Authentication: login, refresh, logout, activate"),
        (name = "admin",  description = "Admin-only operations"),
        (name = "collections", description = "Collection management"),
        (name = "pages",  description = "Page authoring and publishing"),
        (name = "editor", description = "AI-assisted document drafting"),
        (name = "export", description = "Markdown export"),
    )
)]
pub struct ApiDoc;

/// Registers the "bearer" security scheme so Swagger UI shows the
/// "Authorize" button. Paste the access_token from POST /auth/login.
struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}
