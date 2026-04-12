// Hand-written TypeScript types mirroring the Rust DTOs.
// The generated types in packages/types are incomplete (missing
// pages/collections/editor endpoints added in Sprints 3-4).

export type Role = "admin" | "author" | "viewer";
export type PageStatus = "draft" | "published";
export type LlmProvider = "openai" | "anthropic" | "ollama" | "test";

// ---- Auth ----

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface JwtPayload {
  sub: string; // user_id
  wsid: string; // workspace_id
  role: Role;
  exp: number;
  iat: number;
}

// ---- Setup ----

export interface SetupRequest {
  admin_email: string;
  admin_password: string;
  workspace_name: string;
  llm_provider: LlmProvider;
  llm_api_key: string;
  languages: string[];
  primary_language: string;
}

export interface SetupResponse {
  workspace_id: string;
  user_id: string;
  setup_complete: boolean;
}

export interface ProbeRequest {
  llm_provider: LlmProvider;
  llm_api_key: string;
}

export interface ProbeResponse {
  success: boolean;
  message: string;
}

// ---- Collections ----

export interface Collection {
  id: string;
  workspace_id: string;
  parent_id: string | null;
  name: string;
  slug: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface TreeNode extends Collection {
  children: TreeNode[];
}

// ---- Pages ----

export interface PageVersion {
  id: string;
  language: string;
  title: string;
  content_markdown: string;
  status: PageStatus;
  author_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface PageResponse {
  id: string;
  workspace_id: string;
  collection_id: string | null;
  slug: string;
  status: PageStatus;
  created_by: string | null;
  versions: PageVersion[];
  created_at: string;
  updated_at: string;
}

// ---- Users ----

export interface UserResponse {
  id: string;
  email: string;
  role: Role;
  active: boolean;
  pending: boolean;
}

export interface InviteResponse {
  user_id: string;
  activation_url: string;
  expires_at: string;
}

// ---- Workspace ----

export interface WorkspaceResponse {
  id: string;
  name: string;
  languages: string[];
  primary_language: string;
  llm_provider: string;
  mcp_endpoint_url: string;
  has_mcp_token: boolean;
}

export interface RegenerateTokenResponse {
  bearer_token: string;
}
