use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::{admin, auth, health, setup};

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
