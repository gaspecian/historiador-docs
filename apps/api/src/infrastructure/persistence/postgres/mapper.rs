//! Row → domain entity conversions. Kept in one place so the mapping
//! rules are easy to audit when the schema evolves.

use historiador_db::postgres::{
    collections as c_rows, installation as i_rows, page_version_history as vh_rows,
    page_versions as pv_rows, pages as p_rows, sessions as s_rows, users as u_rows,
    workspaces as w_rows,
};

use crate::domain::entity::{
    Collection, Installation, Page, PageVersion, Session, User, VersionHistoryEntry,
    VersionHistorySummary, Workspace,
};
use crate::domain::value::{Email, Language, PageStatus, Role, Slug};

pub fn page(row: p_rows::Page) -> Page {
    Page {
        id: row.id,
        workspace_id: row.workspace_id,
        collection_id: row.collection_id,
        slug: Slug::from_trusted(row.slug),
        status: page_status(row.status),
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn page_version(row: pv_rows::PageVersion) -> PageVersion {
    PageVersion {
        id: row.id,
        page_id: row.page_id,
        language: Language::from_trusted(row.language),
        title: row.title,
        content_markdown: row.content_markdown,
        status: page_status(row.status),
        author_id: row.author_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn collection(row: c_rows::Collection) -> Collection {
    Collection {
        id: row.id,
        workspace_id: row.workspace_id,
        parent_id: row.parent_id,
        name: row.name,
        slug: Slug::from_trusted(row.slug),
        sort_order: row.sort_order,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn user(row: u_rows::User) -> User {
    User {
        id: row.id,
        workspace_id: row.workspace_id,
        email: Email::from_trusted(row.email),
        password_hash: row.password_hash,
        role: role(row.role),
        active: row.active,
        invite_token_hash: row.invite_token_hash,
        invite_expires_at: row.invite_expires_at,
    }
}

pub fn workspace(row: w_rows::Workspace) -> Workspace {
    let languages = row
        .languages
        .into_iter()
        .map(Language::from_trusted)
        .collect();
    Workspace {
        id: row.id,
        name: row.name,
        languages,
        primary_language: Language::from_trusted(row.primary_language),
        llm_provider: row.llm_provider,
        llm_api_key_encrypted: row.llm_api_key_encrypted,
        llm_base_url: row.llm_base_url,
        generation_model: row.generation_model,
        embedding_model: row.embedding_model,
        mcp_bearer_token_hash: row.mcp_bearer_token_hash,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn session(row: s_rows::Session) -> Session {
    Session {
        id: row.id,
        user_id: row.user_id,
        expires_at: row.expires_at,
    }
}

pub fn installation(row: i_rows::Installation) -> Installation {
    Installation {
        setup_complete: row.setup_complete,
        completed_at: row.completed_at,
    }
}

pub fn version_history_entry(row: vh_rows::PageVersionHistoryRow) -> VersionHistoryEntry {
    VersionHistoryEntry {
        id: row.id,
        page_id: row.page_id,
        language: Language::from_trusted(row.language),
        title: row.title,
        content_markdown: row.content_markdown,
        is_published: row.is_published,
        author_id: row.author_id,
        version_number: row.version_number,
        created_at: row.created_at,
    }
}

pub fn version_history_summary(
    row: vh_rows::PageVersionHistorySummary,
) -> VersionHistorySummary {
    VersionHistorySummary {
        id: row.id,
        version_number: row.version_number,
        title: row.title,
        content_preview: row.content_preview,
        is_published: row.is_published,
        author_id: row.author_id,
        created_at: row.created_at,
    }
}

pub fn page_status(status: p_rows::PageStatus) -> PageStatus {
    match status {
        p_rows::PageStatus::Draft => PageStatus::Draft,
        p_rows::PageStatus::Published => PageStatus::Published,
    }
}

pub fn page_status_to_db(status: PageStatus) -> p_rows::PageStatus {
    match status {
        PageStatus::Draft => p_rows::PageStatus::Draft,
        PageStatus::Published => p_rows::PageStatus::Published,
    }
}

pub fn role(role: u_rows::Role) -> Role {
    match role {
        u_rows::Role::Admin => Role::Admin,
        u_rows::Role::Author => Role::Author,
        u_rows::Role::Viewer => Role::Viewer,
    }
}

pub fn role_to_db(role: Role) -> u_rows::Role {
    match role {
        Role::Admin => u_rows::Role::Admin,
        Role::Author => u_rows::Role::Author,
        Role::Viewer => u_rows::Role::Viewer,
    }
}
