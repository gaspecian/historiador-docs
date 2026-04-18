// Collections service — /collections/* CRUD.

import { apiFetch } from "../api";
import type { CollectionResponse } from "@historiador/types";

export interface CreateCollectionBody {
  name: string;
  parent_id?: string | null;
}

export interface UpdateCollectionBody {
  name?: string;
  /** `null` moves to root; omit to leave unchanged. */
  parent_id?: string | null;
}

export async function list(): Promise<CollectionResponse[]> {
  return apiFetch<CollectionResponse[]>("/collections");
}

export async function create(body: CreateCollectionBody): Promise<CollectionResponse> {
  return apiFetch<CollectionResponse>("/collections", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function update(
  id: string,
  body: UpdateCollectionBody,
): Promise<CollectionResponse> {
  return apiFetch<CollectionResponse>(`/collections/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
}

export async function remove(id: string): Promise<void> {
  return apiFetch<void>(`/collections/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}
