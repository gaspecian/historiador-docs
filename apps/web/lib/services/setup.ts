// Setup service — first-run wizard operations.

import { apiFetch } from "../api";
import type {
  ProbeRequest,
  ProbeResponse,
  SetupRequest,
  SetupResponse,
} from "@historiador/types";

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
