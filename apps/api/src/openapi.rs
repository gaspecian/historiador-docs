use utoipa::OpenApi;

use crate::health;

#[derive(OpenApi)]
#[openapi(
    paths(health::handler),
    components(schemas(health::HealthResponse)),
    info(
        title = "Historiador Doc API",
        version = "0.1.0",
        description = "REST API for Historiador Doc — self-hosted documentation with a built-in MCP server."
    ),
    tags(
        (name = "system", description = "Health and system metadata")
    )
)]
pub struct ApiDoc;
