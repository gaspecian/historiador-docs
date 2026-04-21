// Setup service — first-run wizard operations.

import { apiFetch } from "../api";
import type {
  ProbeRequest,
  ProbeResponse,
  SetupRequest,
  SetupResponse,
  SetupStatusResponse,
} from "@historiador/types";

// Uses raw fetch — apiFetch redirects to /setup on any 423, which would
// mask the very signal we're trying to read.
export async function status(): Promise<SetupStatusResponse> {
  const res = await fetch("/api/setup/status");
  if (!res.ok) throw new Error(`setup status failed: ${res.status}`);
  return res.json();
}

export async function init(body: SetupRequest): Promise<SetupResponse> {
  return apiFetch<SetupResponse>("/setup/init", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function probe(body: ProbeRequest): Promise<ProbeResponse> {
  return apiFetch<ProbeResponse>("/setup/probe", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export interface OllamaModel {
  name: string;
  size_bytes: number;
}

export async function ollamaModels(baseUrl: string): Promise<OllamaModel[]> {
  const res = await apiFetch<{ models: OllamaModel[] }>("/setup/ollama-models", {
    method: "POST",
    body: JSON.stringify({ base_url: baseUrl }),
  });
  return res.models;
}
