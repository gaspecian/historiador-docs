"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import * as adminService from "@/lib/services/admin";
import * as setupService from "@/lib/services/setup";
import type { LlmProvider, WorkspaceResponse } from "@historiador/types";

const VALID_PROVIDERS: LlmProvider[] = ["openai", "anthropic", "ollama", "test"];

const PROVIDER_OPTIONS = [
  { value: "", label: "Select a provider…" },
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama (local)" },
  { value: "test", label: "Test (no LLM)" },
];

type ProviderState = LlmProvider | "";

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

const initialProviderFor = (workspace: WorkspaceResponse): ProviderState =>
  VALID_PROVIDERS.includes(workspace.llm_provider as LlmProvider)
    ? (workspace.llm_provider as LlmProvider)
    : "";

export function LlmSettingsForm({ workspace, onSaved }: Props) {
  const [provider, setProvider] = useState<ProviderState>(initialProviderFor(workspace));
  // Empty means "keep the existing encrypted key / base URL on the server."
  const [apiKey, setApiKey] = useState("");
  const [generationModel, setGenerationModel] = useState<string>(workspace.generation_model ?? "");
  const [embeddingModel, setEmbeddingModel] = useState<string>(workspace.embedding_model ?? "");
  const [probeMessage, setProbeMessage] = useState<string | null>(null);
  const [probeSuccess, setProbeSuccess] = useState<boolean | null>(null);
  const [ollamaModels, setOllamaModels] = useState<OllamaModelEntry[]>([]);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<LlmPatchResponse | null>(null);
  const [reindexStatus, setReindexStatus] = useState<string | null>(null);

  // Sync local state to the canonical workspace prop. Without this,
  // after save → onSaved() → refresh, the form keeps its old local
  // state and the NEXT save would PATCH stale values over the fresh
  // DB row. Triggered on workspace prop identity change.
  useEffect(() => {
    setProvider(initialProviderFor(workspace));
    setGenerationModel(workspace.generation_model ?? "");
    setEmbeddingModel(workspace.embedding_model ?? "");
    setApiKey("");
    setProbeMessage(null);
    setProbeSuccess(null);
    setOllamaModels([]);
  }, [workspace]);

  const handleProviderChange = (p: ProviderState) => {
    setProvider(p);
    setApiKey("");
    // Keep workspace models only when staying on the same provider; otherwise
    // clear so we never PATCH stale model names from a different provider
    // (e.g. carrying "stub" from the test provider over to ollama).
    const sameProvider = p === workspace.llm_provider;
    setGenerationModel(sameProvider ? workspace.generation_model ?? "" : "");
    setEmbeddingModel(sameProvider ? workspace.embedding_model ?? "" : "");
    setProbeMessage(null);
    setProbeSuccess(null);
    setOllamaModels([]);
    setResult(null);
    setReindexStatus(null);
  };

  // When Ollama models load (after a successful probe), auto-select the first
  // one if the current selection isn't in the list. Without this, the <select>
  // visually shows the first option but React state stays empty, so save would
  // omit the field and the backend would keep the prior value.
  useEffect(() => {
    if (provider !== "ollama" || ollamaModels.length === 0) return;
    const names = ollamaModels.map((m) => m.name);
    if (!generationModel || !names.includes(generationModel)) {
      setGenerationModel(ollamaModels[0].name);
    }
    if (!embeddingModel || !names.includes(embeddingModel)) {
      setEmbeddingModel(ollamaModels[0].name);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ollamaModels, provider]);

  const testConnection = async () => {
    if (provider === "" || provider === "test") return;
    setTesting(true);
    setProbeMessage(null);
    setProbeSuccess(null);
    try {
      const data = await setupService.probe({
        llm_provider: provider,
        llm_api_key: apiKey,
      });
      setProbeSuccess(data.success);
      setProbeMessage(data.message);
      if (data.success && provider === "ollama") {
        const models = await setupService.ollamaModels(apiKey);
        setOllamaModels(models);
      }
    } catch (e) {
      setProbeSuccess(false);
      setProbeMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setTesting(false);
    }
  };

  const save = async () => {
    if (provider === "") return;
    if (provider !== "test" && (!generationModel.trim() || !embeddingModel.trim())) {
      setProbeSuccess(false);
      setProbeMessage("Generation and embedding model are required.");
      return;
    }
    setSaving(true);
    setResult(null);
    try {
      const res = await adminService.updateLlmConfig({
        llm_provider: provider,
        llm_api_key: apiKey, // empty string ⇒ keep existing secret
        generation_model: generationModel,
        embedding_model: embeddingModel,
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
      const r = await adminService.reindex();
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

  const probeBlock = probeMessage && (
    <p className={`text-sm ${probeSuccess ? "text-teal-600" : "text-red-600"}`}>
      {probeMessage}
    </p>
  );

  const saveBlock = (
    <div className="flex gap-2">
      <Button onClick={save} disabled={saving}>
        {saving ? <><Spinner className="mr-2" /> Saving…</> : "Save LLM settings"}
      </Button>
    </div>
  );

  const resultBlock = result?.success && (
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
  );

  const renderApiKeyProviderForm = (label: "OpenAI" | "Anthropic") => (
    <div className="space-y-4">
      <Input
        label={`${label} API key (leave empty to keep)`}
        type="password"
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        placeholder="•••••••• (keep existing)"
      />
      <Button
        variant="secondary"
        size="sm"
        onClick={testConnection}
        disabled={testing || !apiKey.trim()}
      >
        {testing ? <><Spinner className="mr-2" /> Testing…</> : "Test connection"}
      </Button>
      {probeBlock}
      <div className="grid grid-cols-2 gap-4">
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
      </div>
      {saveBlock}
      {resultBlock}
    </div>
  );

  const renderOllamaForm = () => (
    <div className="space-y-4">
      <Input
        label="Ollama base URL"
        type="url"
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        placeholder={workspace.llm_base_url ?? "http://localhost:11434"}
      />
      <Button
        variant="secondary"
        size="sm"
        onClick={testConnection}
        disabled={testing || !apiKey.trim()}
      >
        {testing ? <><Spinner className="mr-2" /> Testing…</> : "Test connection"}
      </Button>
      {probeBlock}
      <div className="grid grid-cols-2 gap-4">
        {ollamaOptions.length > 0 ? (
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
      {saveBlock}
      {resultBlock}
    </div>
  );

  const renderTestForm = () => (
    <div className="space-y-4">
      <p className="text-sm text-text-secondary">
        The test provider returns deterministic stub responses. No credentials or
        models required — useful for local development and CI.
      </p>
      {saveBlock}
      {resultBlock}
    </div>
  );

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <Select
          label="Provider"
          options={PROVIDER_OPTIONS}
          value={provider}
          onChange={(e) => handleProviderChange(e.target.value as ProviderState)}
        />
      </div>

      {provider === "openai" && renderApiKeyProviderForm("OpenAI")}
      {provider === "anthropic" && renderApiKeyProviderForm("Anthropic")}
      {provider === "ollama" && renderOllamaForm()}
      {provider === "test" && renderTestForm()}
    </div>
  );
}
