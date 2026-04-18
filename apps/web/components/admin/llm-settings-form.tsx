"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { apiFetch } from "@/lib/api";
import type { LlmProvider, WorkspaceResponse } from "@historiador/types";

const PROVIDER_OPTIONS = [
 { value: "openai", label: "OpenAI" },
 { value: "anthropic", label: "Anthropic" },
 { value: "ollama", label: "Ollama (local)" },
 { value: "test", label: "Test (no LLM)" },
];

interface OllamaModelEntry {
 name: string;
 size_bytes: number;
}

interface LlmPatchResponse {
 success: boolean;
 requires_reindex: boolean;
 affected_page_versions: number;
 requires_restart: boolean;
}

interface Props {
 workspace: WorkspaceResponse;
 onSaved: () => void;
}

export function LlmSettingsForm({ workspace, onSaved }: Props) {
 const [provider, setProvider] = useState<LlmProvider>(
 workspace.llm_provider as LlmProvider,
 );
 // Empty means "keep the existing encrypted key / base URL on the server."
 const [apiKey, setApiKey] = useState("");
 const [generationModel, setGenerationModel] = useState(workspace.generation_model);
 const [embeddingModel, setEmbeddingModel] = useState(workspace.embedding_model);
 const [probeMessage, setProbeMessage] = useState<string | null>(null);
 const [probeSuccess, setProbeSuccess] = useState<boolean | null>(null);
 const [ollamaModels, setOllamaModels] = useState<OllamaModelEntry[]>([]);
 const [saving, setSaving] = useState(false);
 const [testing, setTesting] = useState(false);
 const [result, setResult] = useState<LlmPatchResponse | null>(null);
 const [reindexStatus, setReindexStatus] = useState<string | null>(null);

 const handleProviderChange = (p: LlmProvider) => {
 setProvider(p);
 setApiKey("");
 setProbeMessage(null);
 setProbeSuccess(null);
 setOllamaModels([]);
 setResult(null);
 setReindexStatus(null);
 };

 const testConnection = async () => {
 setTesting(true);
 setProbeMessage(null);
 setProbeSuccess(null);
 try {
 const data = await apiFetch<{ success: boolean; message: string }>(
 "/setup/probe",
 {
 method: "POST",
 body: JSON.stringify({ llm_provider: provider, llm_api_key: apiKey }),
 },
 );
 setProbeSuccess(data.success);
 setProbeMessage(data.message);
 if (data.success && provider === "ollama") {
 const models = await apiFetch<{ models: OllamaModelEntry[] }>(
 "/setup/ollama-models",
 {
 method: "POST",
 body: JSON.stringify({ base_url: apiKey }),
 },
 );
 setOllamaModels(models.models);
 }
 } catch (e) {
 setProbeSuccess(false);
 setProbeMessage(e instanceof Error ? e.message : String(e));
 } finally {
 setTesting(false);
 }
 };

 const save = async () => {
 setSaving(true);
 setResult(null);
 try {
 const res = await apiFetch<LlmPatchResponse>("/admin/workspace/llm", {
 method: "PATCH",
 body: JSON.stringify({
 llm_provider: provider,
 llm_api_key: apiKey, // empty string ⇒ keep existing secret
 generation_model: generationModel,
 embedding_model: embeddingModel,
 }),
 });
 setResult(res);
 onSaved();
 } catch (e) {
 setResult({
 success: false,
 requires_reindex: false,
 affected_page_versions: 0,
 requires_restart: false,
 });
 setProbeMessage(e instanceof Error ? e.message : String(e));
 } finally {
 setSaving(false);
 }
 };

 const triggerReindex = async () => {
 setReindexStatus("Re-indexing…");
 try {
 const r = await apiFetch<{ scheduled: number }>("/admin/workspace/reindex", {
 method: "POST",
 });
 setReindexStatus(`Re-indexing ${r.scheduled} page version(s) in background.`);
 } catch (e) {
 setReindexStatus(
 `Failed: ${e instanceof Error ? e.message : String(e)}`,
 );
 }
 };

 const ollamaOptions = ollamaModels.map((m) => ({
 value: m.name,
 label: `${m.name} (${(m.size_bytes / 1e9).toFixed(1)} GB)`,
 }));

 return (
 <div className="space-y-4">
 <div className="grid grid-cols-2 gap-4">
 <Select
 label="Provider"
 options={PROVIDER_OPTIONS}
 value={provider}
 onChange={(e) => handleProviderChange(e.target.value as LlmProvider)}
 />
 {provider !== "test" && (
 <Input
 label={provider === "ollama" ? "Ollama base URL" : "API key (leave empty to keep)"}
 type={provider === "ollama" ? "url" : "password"}
 value={apiKey}
 onChange={(e) => setApiKey(e.target.value)}
 placeholder={
 provider === "ollama"
 ? workspace.llm_base_url ?? "http://localhost:11434"
 : "•••••••• (keep existing)"
 }
 />
 )}
 </div>

 {provider !== "test" && (
 <Button
 variant="secondary"
 size="sm"
 onClick={testConnection}
 disabled={testing || !apiKey.trim()}
 >
 {testing ? <><Spinner className="mr-2" /> Testing…</> : "Test connection"}
 </Button>
 )}
 {probeMessage && (
 <p className={`text-sm ${probeSuccess ? "text-teal-600" : "text-red-600"}`}>
 {probeMessage}
 </p>
 )}

 <div className="grid grid-cols-2 gap-4">
 {provider === "ollama" && ollamaOptions.length > 0 ? (
 <>
 <Select
 label="Generation model"
 options={ollamaOptions}
 value={generationModel}
 onChange={(e) => setGenerationModel(e.target.value)}
 />
 <Select
 label="Embedding model"
 options={ollamaOptions}
 value={embeddingModel}
 onChange={(e) => setEmbeddingModel(e.target.value)}
 />
 </>
 ) : (
 <>
 <Input
 label="Generation model"
 value={generationModel}
 onChange={(e) => setGenerationModel(e.target.value)}
 />
 <Input
 label="Embedding model"
 value={embeddingModel}
 onChange={(e) => setEmbeddingModel(e.target.value)}
 />
 </>
 )}
 </div>

 <div className="flex gap-2">
 <Button onClick={save} disabled={saving}>
 {saving ? <><Spinner className="mr-2" /> Saving…</> : "Save LLM settings"}
 </Button>
 </div>

 {result?.success && (
 <div className="space-y-2 rounded border border-surface-border p-3 text-sm">
 <p className="text-teal-600">Settings saved.</p>
 {result.requires_restart && (
 <p className="text-amber-600">
 Restart the API process for the generation model change to take effect in the
 editor.
 </p>
 )}
 {result.requires_reindex && (
 <div className="space-y-2">
 <p className="text-amber-600">
 Changing the embedding model requires re-embedding{" "}
 {result.affected_page_versions} published page version(s). Until re-indexed,
 MCP queries will mismatch and return no results.
 </p>
 <Button size="sm" variant="danger" onClick={triggerReindex}>
 Re-index now
 </Button>
 </div>
 )}
 {reindexStatus && <p className="text-sm">{reindexStatus}</p>}
 </div>
 )}
 </div>
 );
}
