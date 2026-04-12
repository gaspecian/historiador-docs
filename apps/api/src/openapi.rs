use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::{admin, auth, collections, editor, health, pages, setup};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::handler,
        setup::handler::init,
        auth::handlers::login,
        auth::handlers::refresh,
        auth::handlers::logout,
        auth::handlers::activate,
        admin::users::invite,
        collections::handlers::create_collection,
        collections::handlers::list_collections,
        collections::handlers::update_collection,
        collections::handlers::delete_collection,
        pages::handlers::create_page,
        pages::handlers::get_page,
        pages::handlers::update_page,
        pages::handlers::publish_page,
        pages::handlers::draft_page,
        editor::handlers::draft,
        editor::handlers::iterate,
    ),
    components(schemas(
        health::HealthResponse,
        setup::handler::SetupRequest,
        setup::handler::SetupResponse,
        setup::llm_probe::LlmProvider,
        auth::handlers::LoginRequest,
        auth::handlers::TokenResponse,
        auth::handlers::RefreshRequest,
        auth::handlers::LogoutRequest,
        auth::handlers::ActivateRequest,
        admin::users::InviteRequest,
        admin::users::InviteResponse,
        collections::handlers::CreateCollectionRequest,
        collections::handlers::UpdateCollectionRequest,
        collections::handlers::CollectionResponse,
        pages::handlers::CreatePageRequest,
        pages::handlers::UpdatePageRequest,
        pages::handlers::PageResponse,
        pages::handlers::PageVersionResponse,
        pages::handlers::PublishResponse,
        historiador_db::postgres::pages::PageStatus,
        historiador_db::postgres::collections::Collection,
        editor::handlers::DraftRequest,
        editor::handlers::DraftResponse,
        editor::handlers::IterateRequest,
        editor::handlers::IterateResponse,
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
