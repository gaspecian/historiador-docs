// Admin service — user management, workspace config, MCP analytics.

import { apiFetch } from "../api";
import type {
  InviteRequest,
  InviteResponse,
  McpAnalyticsResponse,
  RegenerateTokenResponse,
  UserResponse,
  WorkspaceResponse,
} from "@historiador/types";

// ---- users ----

export async function listUsers(): Promise<UserResponse[]> {
  return apiFetch<UserResponse[]>("/admin/users");
}

export async function invite(body: InviteRequest): Promise<InviteResponse> {
  return apiFetch<InviteResponse>("/admin/users/invite", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function deactivateUser(userId: string): Promise<void> {
  return apiFetch<void>(`/admin/users/${encodeURIComponent(userId)}/deactivate`, {
    method: "PATCH",
  });
}

// ---- workspace ----

export async function getWorkspace(): Promise<WorkspaceResponse> {
  return apiFetch<WorkspaceResponse>("/admin/workspace");
}

export async function regenerateToken(): Promise<RegenerateTokenResponse> {
  return apiFetch<RegenerateTokenResponse>("/admin/workspace/regenerate-token", {
    method: "POST",
  });
}

export interface LlmPatchBody {
  llm_provider: "openai" | "anthropic" | "ollama" | "test";
  llm_api_key?: string;
  generation_model: string;
  embedding_model: string;
}

export interface LlmPatchResult {
  success: boolean;
  requires_reindex: boolean;
  affected_page_versions: number;
  requires_restart: boolean;
}

export async function updateLlmConfig(body: LlmPatchBody): Promise<LlmPatchResult> {
  return apiFetch<LlmPatchResult>("/admin/workspace/llm", {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function reindex(): Promise<{ scheduled: number }> {
  return apiFetch<{ scheduled: number }>("/admin/workspace/reindex", {
    method: "POST",
  });
}

// ---- analytics ----

export async function mcpAnalytics(days?: number): Promise<McpAnalyticsResponse> {
  const qs = days ? `?days=${days}` : "";
  return apiFetch<McpAnalyticsResponse>(`/admin/analytics/mcp-queries${qs}`);
}
