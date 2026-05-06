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
  { value: "", label: "Selecione um provedor…" },
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama (local)" },
  { value: "test", label: "Teste (sem LLM)" },
];

type ProviderState = LlmProvider | "";

interface OllamaModelEntry {
  name: string;
  size_bytes: number;
}

interface LlmPatchResponse {
  success: boolean;
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
  const [baseUrl, setBaseUrl] = useState<string>(workspace.llm_base_url ?? "");
  const [generationModel, setGenerationModel] = useState<string>(workspace.generation_model ?? "");
  const [probeMessage, setProbeMessage] = useState<string | null>(null);
  const [probeSuccess, setProbeSuccess] = useState<boolean | null>(null);
  const [ollamaModels, setOllamaModels] = useState<OllamaModelEntry[]>([]);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<LlmPatchResponse | null>(null);

  // Sync local state to the canonical workspace prop. Without this,
  // after save → onSaved() → refresh, the form keeps its old local
  // state and the NEXT save would PATCH stale values over the fresh
  // DB row. Triggered on workspace prop identity change.
  useEffect(() => {
    setProvider(initialProviderFor(workspace));
    setGenerationModel(workspace.generation_model ?? "");
    setApiKey("");
    setBaseUrl(workspace.llm_base_url ?? "");
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
    // Base URL is OpenAI-only; preserve persisted value when staying on openai,
    // clear otherwise so we never PATCH a URL meant for another provider.
    setBaseUrl(sameProvider ? workspace.llm_base_url ?? "" : "");
    setProbeMessage(null);
    setProbeSuccess(null);
    setOllamaModels([]);
    setResult(null);
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
        base_url:
          provider === "openai" && baseUrl.trim() !== ""
            ? baseUrl.trim()
            : undefined,
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
    if (provider !== "test" && !generationModel.trim()) {
      setProbeSuccess(false);
      setProbeMessage("Modelo de geração obrigatório.");
      return;
    }
    setSaving(true);
    setResult(null);
    try {
      const res = await adminService.updateLlmConfig({
        llm_provider: provider,
        llm_api_key: apiKey, // empty string ⇒ keep existing secret
        base_url:
          provider === "openai" && baseUrl.trim() !== ""
            ? baseUrl.trim()
            : undefined,
        generation_model: generationModel,
      });
      setResult(res);
      onSaved();
    } catch (e) {
      setResult({
        success: false,
        requires_restart: false,
      });
      setProbeMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
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
        {saving ? <><Spinner className="mr-2" /> Salvando…</> : "Salvar configurações de LLM"}
      </Button>
    </div>
  );

  const resultBlock = result?.success && (
    <div className="space-y-2 rounded border border-surface-border p-3 text-sm">
      <p className="text-teal-600">Configurações salvas.</p>
      {result.requires_restart && (
        <p className="text-amber-600">
          Reinicie o processo da API para que a alteração do modelo de geração tenha efeito no editor.
        </p>
      )}
    </div>
  );

  const renderApiKeyProviderForm = (label: "OpenAI" | "Anthropic") => {
    const isOpenAi = label === "OpenAI";
    // Test connection is enabled when EITHER a key is present OR
    // (for OpenAI only) a base URL is set — covers the no-auth path.
    const canTest = !!apiKey.trim() || (isOpenAi && !!baseUrl.trim());
    return (
      <div className="space-y-4">
        <Input
          label={`Chave de API ${label} (deixe vazio para manter${
            isOpenAi ? " ou para usar sem auth com URL personalizada" : ""
          })`}
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="•••••••• (manter existente)"
        />
        {isOpenAi && (
          <Input
            label="Base URL (opcional)"
            type="url"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.openai.com/v1 (padrão)"
          />
        )}
        <Button
          variant="secondary"
          size="sm"
          onClick={testConnection}
          disabled={testing || !canTest}
        >
          {testing ? <><Spinner className="mr-2" /> Testando…</> : "Testar conexão"}
        </Button>
        {probeBlock}
        <Input
          label="Modelo de geração"
          value={generationModel}
          onChange={(e) => setGenerationModel(e.target.value)}
        />
        {saveBlock}
        {resultBlock}
      </div>
    );
  };

  const renderOllamaForm = () => (
    <div className="space-y-4">
      <Input
        label="URL base do Ollama"
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
        {testing ? <><Spinner className="mr-2" /> Testando…</> : "Testar conexão"}
      </Button>
      {probeBlock}
      {ollamaOptions.length > 0 ? (
        <Select
          label="Modelo de geração"
          options={ollamaOptions}
          value={generationModel}
          onChange={(e) => setGenerationModel(e.target.value)}
        />
      ) : (
        <Input
          label="Modelo de geração"
          value={generationModel}
          onChange={(e) => setGenerationModel(e.target.value)}
        />
      )}
      {saveBlock}
      {resultBlock}
    </div>
  );

  const renderTestForm = () => (
    <div className="space-y-4">
      <p className="text-sm text-text-secondary">
        O provedor de teste retorna respostas determinísticas. Não exige credenciais
        ou modelos — útil para desenvolvimento local e CI.
      </p>
      {saveBlock}
      {resultBlock}
    </div>
  );

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <Select
          label="Provedor"
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
