//! Composition of every use case, built once at boot and shared as
//! `Arc<UseCases>` through `AppState`. Handlers reach for a use case
//! on this struct and never touch infrastructure types directly.
//!
//! Construction is `UseCases::build(BuildDeps)` — a single call that
//! wires every adapter against its port trait. The existing
//! `AppState` already holds the primitive infrastructure it needs
//! (pool, cipher, JWT secret, LLM clients, Chronik); this bundle sits
//! alongside those primitives during the refactor.

use std::sync::Arc;

use sqlx::PgPool;

use historiador_db::chronik::ChronikClient;
use historiador_db::vector_store::VectorStore;
use historiador_llm::{EmbeddingClient, TextGenerationClient};

use crate::application::admin::{
    DeactivateUserUseCase, GetMcpAnalyticsUseCase, GetWorkspaceUseCase, InviteUserUseCase,
    ListUsersUseCase, LogMcpQueryUseCase, RegenerateTokenUseCase, ReindexWorkspaceUseCase,
    UpdateLlmConfigUseCase,
};
use crate::application::auth::{ActivateUseCase, LoginUseCase, LogoutUseCase, RefreshUseCase};
use crate::application::collections::{
    CreateCollectionUseCase, DeleteCollectionUseCase, ListCollectionsUseCase,
    UpdateCollectionUseCase,
};
use crate::application::editor::{GenerateDraftUseCase, IterateDraftUseCase};
use crate::application::export::{ExportPageUseCase, ExportWorkspaceUseCase};
use crate::application::pages::{
    CreatePageUseCase, DraftPageUseCase, GetPageUseCase, GetPageVersionsUseCase,
    GetVersionHistoryItemUseCase, ListPagesUseCase, ListVersionHistoryUseCase, PublishPageUseCase,
    RestoreVersionUseCase, SearchPagesUseCase, UpdatePageUseCase,
};
use crate::application::setup::{
    InitializeInstallationUseCase, ListOllamaModelsUseCase, ProbeLlmUseCase,
};
use crate::domain::port::cipher::Cipher as CipherPort;
use crate::domain::port::llm_probe::LlmProbe;
use crate::infrastructure::chronik::{ChronikEventProducer, ChronikQueryAnalytics};
use crate::infrastructure::chunker::DefaultChunkPipeline;
use crate::infrastructure::crypto::raw::Cipher as AesCipher;
use crate::infrastructure::crypto::AesGcmCipher;
use crate::infrastructure::persistence::postgres::{
    PostgresCollectionRepository, PostgresExportRepository, PostgresInstallationRepository,
    PostgresPageRepository, PostgresSessionRepository, PostgresUserRepository,
    PostgresVersionHistoryRepository, PostgresWorkspaceRepository,
};
use crate::infrastructure::token::JwtTokenIssuer;

/// Everything the composition root needs to hand in. These are the
/// same primitives already held by `AppState`, passed through so the
/// refactor doesn't duplicate their construction.
pub struct BuildDeps {
    pub pool: PgPool,
    pub cipher: AesCipher,
    pub jwt_secret: Vec<u8>,
    pub llm_probe: Arc<dyn LlmProbe>,
    pub vector_store: Arc<dyn VectorStore>,
    pub embedding_client: Arc<dyn EmbeddingClient>,
    pub text_generation_client: Arc<dyn TextGenerationClient>,
    pub chronik: Option<ChronikClient>,
}

/// Use-case bundle. Each field is an `Arc<_>` so handlers can hold a
/// cheap reference and so tests can swap in fakes by rebuilding just
/// the field they care about.
pub struct UseCases {
    // auth
    pub login: Arc<LoginUseCase>,
    pub refresh: Arc<RefreshUseCase>,
    pub logout: Arc<LogoutUseCase>,
    pub activate: Arc<ActivateUseCase>,

    // setup
    pub initialize_installation: Arc<InitializeInstallationUseCase>,
    pub probe_llm: Arc<ProbeLlmUseCase>,
    pub list_ollama_models: Arc<ListOllamaModelsUseCase>,

    // pages
    pub list_pages: Arc<ListPagesUseCase>,
    pub search_pages: Arc<SearchPagesUseCase>,
    pub get_page: Arc<GetPageUseCase>,
    pub create_page: Arc<CreatePageUseCase>,
    pub update_page: Arc<UpdatePageUseCase>,
    pub publish_page: Arc<PublishPageUseCase>,
    pub draft_page: Arc<DraftPageUseCase>,
    pub get_page_versions: Arc<GetPageVersionsUseCase>,
    pub list_version_history: Arc<ListVersionHistoryUseCase>,
    pub get_version_history_item: Arc<GetVersionHistoryItemUseCase>,
    pub restore_version: Arc<RestoreVersionUseCase>,

    // collections
    pub create_collection: Arc<CreateCollectionUseCase>,
    pub list_collections: Arc<ListCollectionsUseCase>,
    pub update_collection: Arc<UpdateCollectionUseCase>,
    pub delete_collection: Arc<DeleteCollectionUseCase>,

    // admin
    pub list_users: Arc<ListUsersUseCase>,
    pub invite_user: Arc<InviteUserUseCase>,
    pub deactivate_user: Arc<DeactivateUserUseCase>,
    pub get_workspace: Arc<GetWorkspaceUseCase>,
    pub regenerate_token: Arc<RegenerateTokenUseCase>,
    pub update_llm_config: Arc<UpdateLlmConfigUseCase>,
    pub reindex_workspace: Arc<ReindexWorkspaceUseCase>,
    pub get_mcp_analytics: Arc<GetMcpAnalyticsUseCase>,
    pub log_mcp_query: Arc<LogMcpQueryUseCase>,

    // editor
    pub generate_draft: Arc<GenerateDraftUseCase>,
    pub iterate_draft: Arc<IterateDraftUseCase>,

    // export
    pub export_workspace: Arc<ExportWorkspaceUseCase>,
    pub export_page: Arc<ExportPageUseCase>,
}

impl UseCases {
    pub fn build(deps: BuildDeps) -> Self {
        // ---- repositories ----
        let pages = Arc::new(PostgresPageRepository::new(deps.pool.clone()));
        let collections = Arc::new(PostgresCollectionRepository::new(deps.pool.clone()));
        let users = Arc::new(PostgresUserRepository::new(deps.pool.clone()));
        let workspaces = Arc::new(PostgresWorkspaceRepository::new(deps.pool.clone()));
        let sessions = Arc::new(PostgresSessionRepository::new(deps.pool.clone()));
        let history = Arc::new(PostgresVersionHistoryRepository::new(deps.pool.clone()));
        let _installation = Arc::new(PostgresInstallationRepository::new(deps.pool.clone()));
        let export_repo = Arc::new(PostgresExportRepository::new(deps.pool.clone()));

        // ---- infra adapters ----
        let cipher: Arc<dyn CipherPort> = Arc::new(AesGcmCipher::new(deps.cipher));
        let token_issuer = Arc::new(JwtTokenIssuer::new(deps.jwt_secret));
        let events = Arc::new(ChronikEventProducer::new(deps.chronik.clone()));
        let analytics = Arc::new(ChronikQueryAnalytics::new(deps.chronik));
        let chunk_pipeline = Arc::new(DefaultChunkPipeline::new(
            deps.pool.clone(),
            deps.vector_store.clone(),
            deps.embedding_client.clone(),
        ));

        // ---- use cases ----
        Self {
            // auth
            login: Arc::new(LoginUseCase::new(
                users.clone(),
                sessions.clone(),
                token_issuer.clone(),
            )),
            refresh: Arc::new(RefreshUseCase::new(
                users.clone(),
                sessions.clone(),
                token_issuer.clone(),
            )),
            logout: Arc::new(LogoutUseCase::new(sessions.clone())),
            activate: Arc::new(ActivateUseCase::new(users.clone())),

            // setup
            initialize_installation: Arc::new(InitializeInstallationUseCase::new(
                workspaces.clone(),
                deps.llm_probe.clone(),
                cipher.clone(),
            )),
            probe_llm: Arc::new(ProbeLlmUseCase::new(deps.llm_probe.clone())),
            list_ollama_models: Arc::new(ListOllamaModelsUseCase::new()),

            // pages
            list_pages: Arc::new(ListPagesUseCase::new(pages.clone())),
            search_pages: Arc::new(SearchPagesUseCase::new(pages.clone())),
            get_page: Arc::new(GetPageUseCase::new(pages.clone())),
            create_page: Arc::new(CreatePageUseCase::new(pages.clone(), events.clone())),
            update_page: Arc::new(UpdatePageUseCase::new(
                pages.clone(),
                history.clone(),
                events.clone(),
            )),
            publish_page: Arc::new(PublishPageUseCase::new(
                pages.clone(),
                history.clone(),
                chunk_pipeline.clone(),
                events.clone(),
            )),
            draft_page: Arc::new(DraftPageUseCase::new(
                pages.clone(),
                chunk_pipeline.clone(),
                events.clone(),
            )),
            get_page_versions: Arc::new(GetPageVersionsUseCase::new(
                pages.clone(),
                workspaces.clone(),
            )),
            list_version_history: Arc::new(ListVersionHistoryUseCase::new(
                pages.clone(),
                history.clone(),
            )),
            get_version_history_item: Arc::new(GetVersionHistoryItemUseCase::new(
                pages.clone(),
                history.clone(),
            )),
            restore_version: Arc::new(RestoreVersionUseCase::new(
                pages.clone(),
                history.clone(),
                events.clone(),
            )),

            // collections
            create_collection: Arc::new(CreateCollectionUseCase::new(collections.clone())),
            list_collections: Arc::new(ListCollectionsUseCase::new(collections.clone())),
            update_collection: Arc::new(UpdateCollectionUseCase::new(collections.clone())),
            delete_collection: Arc::new(DeleteCollectionUseCase::new(collections.clone())),

            // admin
            list_users: Arc::new(ListUsersUseCase::new(users.clone())),
            invite_user: Arc::new(InviteUserUseCase::new(users.clone())),
            deactivate_user: Arc::new(DeactivateUserUseCase::new(users.clone())),
            get_workspace: Arc::new(GetWorkspaceUseCase::new(workspaces.clone())),
            regenerate_token: Arc::new(RegenerateTokenUseCase::new(workspaces.clone())),
            update_llm_config: Arc::new(UpdateLlmConfigUseCase::new(
                workspaces.clone(),
                pages.clone(),
                deps.llm_probe.clone(),
                cipher.clone(),
            )),
            reindex_workspace: Arc::new(ReindexWorkspaceUseCase::new(
                workspaces.clone(),
                pages.clone(),
            )),
            get_mcp_analytics: Arc::new(GetMcpAnalyticsUseCase::new(analytics)),
            log_mcp_query: Arc::new(LogMcpQueryUseCase::new(events.clone())),

            // editor
            generate_draft: Arc::new(GenerateDraftUseCase::new(
                deps.text_generation_client.clone(),
            )),
            iterate_draft: Arc::new(IterateDraftUseCase::new(deps.text_generation_client)),

            // export
            export_workspace: Arc::new(ExportWorkspaceUseCase::new(
                workspaces.clone(),
                export_repo.clone(),
            )),
            export_page: Arc::new(ExportPageUseCase::new(workspaces, export_repo)),
        }
    }
}
