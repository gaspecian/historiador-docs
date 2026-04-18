// Pages service — all /pages/* operations as pure async functions.

import { apiFetch } from "../api";
import type {
  CreatePageRequest,
  PageResponse,
  PageVersionsResponse,
  PublishResponse,
  UpdatePageRequest,
  VersionHistoryDetailResponse,
  VersionHistoryListResponse,
} from "@historiador/types";

export async function list(collectionId: string | null): Promise<PageResponse[]> {
  const query = collectionId ? `?collection_id=${encodeURIComponent(collectionId)}` : "";
  return apiFetch<PageResponse[]>(`/pages${query}`);
}

export async function search(query: string): Promise<PageResponse[]> {
  return apiFetch<PageResponse[]>(`/pages/search?q=${encodeURIComponent(query)}`);
}

export async function get(id: string): Promise<PageResponse> {
  return apiFetch<PageResponse>(`/pages/${encodeURIComponent(id)}`);
}

export async function create(body: CreatePageRequest): Promise<PageResponse> {
  return apiFetch<PageResponse>("/pages", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function update(id: string, body: UpdatePageRequest): Promise<PageResponse> {
  return apiFetch<PageResponse>(`/pages/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function publish(id: string): Promise<PublishResponse> {
  return apiFetch<PublishResponse>(`/pages/${encodeURIComponent(id)}/publish`, {
    method: "POST",
  });
}

export async function draft(id: string): Promise<PageResponse> {
  return apiFetch<PageResponse>(`/pages/${encodeURIComponent(id)}/draft`, {
    method: "POST",
  });
}

export async function versions(id: string): Promise<PageVersionsResponse> {
  return apiFetch<PageVersionsResponse>(`/pages/${encodeURIComponent(id)}/versions`);
}

export async function history(
  pageId: string,
  params: { language: string; page?: number; per_page?: number },
): Promise<VersionHistoryListResponse> {
  const qs = new URLSearchParams({ language: params.language });
  if (params.page) qs.set("page", String(params.page));
  if (params.per_page) qs.set("per_page", String(params.per_page));
  return apiFetch<VersionHistoryListResponse>(
    `/pages/${encodeURIComponent(pageId)}/history?${qs.toString()}`,
  );
}

export async function historyItem(
  pageId: string,
  historyId: string,
): Promise<VersionHistoryDetailResponse> {
  return apiFetch<VersionHistoryDetailResponse>(
    `/pages/${encodeURIComponent(pageId)}/history/${encodeURIComponent(historyId)}`,
  );
}

export async function restoreVersion(
  pageId: string,
  historyId: string,
): Promise<PageResponse> {
  return apiFetch<PageResponse>(
    `/pages/${encodeURIComponent(pageId)}/history/${encodeURIComponent(historyId)}/restore`,
    { method: "POST" },
  );
}
