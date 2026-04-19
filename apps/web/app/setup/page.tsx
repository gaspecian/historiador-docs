"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import type { LlmProvider, ProbeResponse } from "@historiador/types";

type Step = "workspace" | "llm" | "admin" | "summary";
const STEPS: Step[] = ["workspace", "llm", "admin", "summary"];

const DEFAULT_PRIMARY_LANGUAGE = "pt-BR";

const PROVIDER_OPTIONS = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama (local)" },
  { value: "test", label: "Teste (sem LLM)" },
];

const DEFAULT_GEN_MODEL: Record<LlmProvider, string> = {
  openai: "gpt-4o-mini",
  anthropic: "claude-haiku-4-5-20251001",
  ollama: "",
  test: "stub",
};

const DEFAULT_EMBED_MODEL: Record<LlmProvider, string> = {
  openai: "text-embedding-3-small",
  anthropic: "text-embedding-3-small",
  ollama: "",
  test: "stub",
};

interface OllamaModelEntry {
  name: string;
  size_bytes: number;
}

export default function SetupPage() {
  const router = useRouter();
  const { login } = useAuth();
  const [step, setStep] = useState<Step>("workspace");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  // Form state
  const [workspaceName, setWorkspaceName] = useState("");
  const [llmProvider, setLlmProvider] = useState<LlmProvider>("openai");
  const [llmApiKey, setLlmApiKey] = useState("");
  const [probeResult, setProbeResult] = useState<ProbeResponse | null>(null);
  const [generationModel, setGenerationModel] = useState(
    DEFAULT_GEN_MODEL.openai,
  );
  const [embeddingModel, setEmbeddingModel] = useState(
    DEFAULT_EMBED_MODEL.openai,
  );
  const [ollamaModels, setOllamaModels] = useState<OllamaModelEntry[]>([]);
  const [ollamaModelsError, setOllamaModelsError] = useState<string | null>(
    null,
  );
  const [adminEmail, setAdminEmail] = useState("");
  const [adminPassword, setAdminPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  const currentIndex = STEPS.indexOf(step);

  const handleProviderChange = (p: LlmProvider) => {
    setLlmProvider(p);
    setProbeResult(null);
    setOllamaModels([]);
    setOllamaModelsError(null);
    setGenerationModel(DEFAULT_GEN_MODEL[p]);
    setEmbeddingModel(DEFAULT_EMBED_MODEL[p]);
  };

  const canGoNext = (): boolean => {
    switch (step) {
      case "workspace":
        return workspaceName.trim().length > 0;
      case "llm":
        if (llmProvider === "test") return true;
        if (!llmApiKey.trim()) return false;
        if (llmProvider === "ollama") {
          // Require a successful probe and both models picked.
          return (
            probeResult?.success === true &&
            generationModel.trim().length > 0 &&
            embeddingModel.trim().length > 0
          );
        }
        return (
          generationModel.trim().length > 0 && embeddingModel.trim().length > 0
        );
      case "admin":
        return (
          adminEmail.includes("@") &&
          adminPassword.length >= 12 &&
          adminPassword === confirmPassword
        );
      case "summary":
        return true;
    }
  };

  const goNext = () => {
    setError("");
    const idx = STEPS.indexOf(step);
    if (idx < STEPS.length - 1) setStep(STEPS[idx + 1]);
  };

  const goBack = () => {
    setError("");
    const idx = STEPS.indexOf(step);
    if (idx > 0) setStep(STEPS[idx - 1]);
  };

  const testConnection = async () => {
    setLoading(true);
    setProbeResult(null);
    setOllamaModels([]);
    setOllamaModelsError(null);
    try {
      const res = await fetch("/api/setup/probe", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          llm_provider: llmProvider,
          llm_api_key: llmApiKey,
        }),
      });
      const data: ProbeResponse = await res.json();
      setProbeResult(data);

      if (data.success && llmProvider === "ollama") {
        const modelsRes = await fetch("/api/setup/ollama-models", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ base_url: llmApiKey }),
        });
        if (modelsRes.ok) {
          const body: { models: OllamaModelEntry[] } = await modelsRes.json();
          setOllamaModels(body.models);
          if (body.models.length > 0) {
            setGenerationModel(body.models[0].name);
            const embedHint = body.models.find((m) =>
              /embed|nomic|mxbai|bge/i.test(m.name),
            );
            setEmbeddingModel((embedHint ?? body.models[0]).name);
          }
        } else {
          const msg = await modelsRes.text().catch(() => "");
          setOllamaModelsError(
            msg || `Failed to list models (HTTP ${modelsRes.status})`,
          );
        }
      }
    } catch (err) {
      setProbeResult({
        success: false,
        message: err instanceof Error ? err.message : "Connection failed",
      });
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async () => {
    setError("");
    setLoading(true);

    try {
      const res = await fetch("/api/setup/init", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          admin_email: adminEmail,
          admin_password: adminPassword,
          workspace_name: workspaceName,
          llm_provider: llmProvider,
          llm_api_key: llmProvider === "test" ? "test" : llmApiKey,
          generation_model: generationModel || undefined,
          embedding_model: embeddingModel || undefined,
          languages: [DEFAULT_PRIMARY_LANGUAGE],
          primary_language: DEFAULT_PRIMARY_LANGUAGE,
        }),
      });

      if (!res.ok) {
        const body = await res.json();
        throw new Error(body.message || "Setup failed");
      }

      await res.json();

      // Auto-login with the admin credentials
      await login(adminEmail, adminPassword);
      router.push("/dashboard/pages");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Setup failed");
    } finally {
      setLoading(false);
    }
  };

  const ollamaModelOptions = ollamaModels.map((m) => ({
    value: m.name,
    label: `${m.name} (${(m.size_bytes / 1e9).toFixed(1)} GB)`,
  }));

  return (
    <main className="grid h-screen grid-cols-1 md:grid-cols-2">
      <aside className="relative hidden flex-col gap-6 bg-primary-600 p-10 text-white md:flex">
        <div
          className="flex items-center gap-2.5"
          style={{
            fontFamily: "var(--font-display)",
            fontStyle: "italic",
            fontSize: 22,
          }}
        >
          <svg
            width={24}
            height={24}
            viewBox="0 0 32 32"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M16 7 V25" />
            <path d="M16 7 C 12 5, 8 5, 4.5 6 V 24 C 8 23, 12 23, 16 25" />
            <path d="M16 7 C 20 5, 24 5, 27.5 6 V 24 C 24 23, 20 23, 16 25" />
            <path d="M22 10 L 26 14" />
          </svg>
          Historiador{" "}
          <span
            style={{
              fontFamily: "var(--font-sans)",
              fontStyle: "normal",
              fontSize: 18,
              fontWeight: 500,
            }}
          >
            Doc
          </span>
        </div>

        <div className="max-w-[440px] text-[16px] leading-relaxed opacity-85 mb-auto">
          Toda equipe tem memória. O Historiador Doc transforma essa memória em
          páginas — conversando com você.
        </div>

        <h1
          className="mt-auto mb-0"
          style={{
            fontFamily: "var(--font-display)",
            fontSize: 52,
            lineHeight: 1.05,
            fontWeight: 400,
            margin: 0,
          }}
        >
          Documentação <em>que conversa</em> de volta.
        </h1>

        <div className="flex gap-1.5">
          {STEPS.map((s, i) => (
            <div
              key={s}
              className="h-[3px] w-8 rounded-sm"
              style={{
                background:
                  i <= currentIndex ? "white" : "rgba(255,255,255,0.25)",
              }}
            />
          ))}
        </div>
      </aside>

      <section className="flex flex-col justify-center bg-surface-canvas p-6 sm:p-10 md:p-16">
        <div className="mx-auto w-full max-w-[520px]">
        <div
          className="mb-2.5 text-[11px] font-bold uppercase text-primary-600"
          style={{ letterSpacing: "0.08em" }}
        >
          Passo {currentIndex + 1} de {STEPS.length}
        </div>

        <div className="space-y-4">
          {/* Step: Workspace */}
          {step === "workspace" && (
            <>
              <h2 className="text-lg font-semibold">Nome do workspace</h2>
              <p className="text-sm text-text-tertiary">
                Escolha um nome para seu workspace de documentação.
              </p>
              <Input
                label="Nome do workspace"
                value={workspaceName}
                onChange={(e) => setWorkspaceName(e.target.value)}
                placeholder="Meus Docs"
                autoFocus
              />
            </>
          )}

          {/* Step: LLM */}
          {step === "llm" && (
            <>
              <h2 className="text-lg font-semibold">Provedor de LLM</h2>
              <p className="text-sm text-text-tertiary">
                Selecione seu provedor de IA para o editor de documentos.
              </p>
              <Select
                label="Provedor"
                options={PROVIDER_OPTIONS}
                value={llmProvider}
                onChange={(e) =>
                  handleProviderChange(e.target.value as LlmProvider)
                }
              />
              {llmProvider !== "test" && (
                <>
                  <Input
                    label={
                      llmProvider === "ollama"
                        ? "URL base do Ollama"
                        : "Chave de API"
                    }
                    type={llmProvider === "ollama" ? "url" : "password"}
                    value={llmApiKey}
                    onChange={(e) => {
                      setLlmApiKey(e.target.value);
                      setProbeResult(null);
                      if (llmProvider === "ollama") setOllamaModels([]);
                    }}
                    placeholder={
                      llmProvider === "ollama"
                        ? "http://localhost:11434"
                        : "sk-..."
                    }
                  />
                  <Button
                    variant="secondary"
                    onClick={testConnection}
                    disabled={loading || !llmApiKey.trim()}
                  >
                    {loading ? (
                      <>
                        <Spinner className="mr-2" /> Testando...
                      </>
                    ) : (
                      "Testar conexão"
                    )}
                  </Button>
                  {probeResult && (
                    <p
                      className={`text-sm ${probeResult.success ? "text-teal-600" : "text-red-600"}`}
                    >
                      {probeResult.success
                        ? "Conexão bem-sucedida"
                        : probeResult.message}
                    </p>
                  )}

                  {/* Model pickers */}
                  {llmProvider === "ollama" && ollamaModels.length > 0 && (
                    <>
                      <Select
                        label="Modelo de geração"
                        options={ollamaModelOptions}
                        value={generationModel}
                        onChange={(e) => setGenerationModel(e.target.value)}
                      />
                      <Select
                        label="Modelo de embedding"
                        options={ollamaModelOptions}
                        value={embeddingModel}
                        onChange={(e) => setEmbeddingModel(e.target.value)}
                      />
                      <p className="text-xs text-text-tertiary">
                        Precisa de mais modelos? Execute{" "}
                        <code>ollama pull &lt;nome&gt;</code> e teste a conexão
                        novamente.
                      </p>
                    </>
                  )}
                  {llmProvider === "ollama" &&
                    probeResult?.success &&
                    !ollamaModelsError &&
                    ollamaModels.length === 0 && (
                      <p className="text-sm text-amber-600">
                        Nenhum modelo instalado neste servidor Ollama. Execute{" "}
                        <code>ollama pull &lt;nome&gt;</code> (ex.{" "}
                        <code>ollama pull llama3</code>) e teste a conexão
                        novamente.
                      </p>
                    )}
                  {llmProvider === "ollama" && ollamaModelsError && (
                    <p className="text-sm text-red-600">
                      Não foi possível listar os modelos do Ollama:{" "}
                      {ollamaModelsError}
                    </p>
                  )}
                  {llmProvider !== "ollama" && (
                    <>
                      <Input
                        label="Modelo de geração"
                        value={generationModel}
                        onChange={(e) => setGenerationModel(e.target.value)}
                        placeholder={DEFAULT_GEN_MODEL[llmProvider]}
                      />
                      <Input
                        label="Modelo de embedding"
                        value={embeddingModel}
                        onChange={(e) => setEmbeddingModel(e.target.value)}
                        placeholder={DEFAULT_EMBED_MODEL[llmProvider]}
                      />
                    </>
                  )}
                </>
              )}
            </>
          )}

          {/* Step: Admin Account */}
          {step === "admin" && (
            <>
              <h2 className="text-lg font-semibold">Conta de administrador</h2>
              <p className="text-sm text-text-tertiary">
                Crie a primeira conta de administrador.
              </p>
              <Input
                label="E-mail"
                type="email"
                value={adminEmail}
                onChange={(e) => setAdminEmail(e.target.value)}
                autoComplete="email"
              />
              <Input
                label="Senha"
                type="password"
                value={adminPassword}
                onChange={(e) => setAdminPassword(e.target.value)}
                placeholder="Mín. 12 caracteres"
                autoComplete="new-password"
                error={
                  adminPassword.length > 0 && adminPassword.length < 12
                    ? "Mín. 12 caracteres"
                    : undefined
                }
              />
              <Input
                label="Confirmar senha"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                autoComplete="new-password"
                error={
                  confirmPassword.length > 0 &&
                  confirmPassword !== adminPassword
                    ? "As senhas não coincidem"
                    : undefined
                }
              />
            </>
          )}

          {/* Step: Summary */}
          {step === "summary" && (
            <>
              <h2 className="text-lg font-semibold">Revisar e concluir</h2>
              <div className="rounded border border-surface-border p-4 space-y-2 text-sm">
                <div>
                  <span className="font-medium">Workspace:</span>{" "}
                  {workspaceName}
                </div>
                <div>
                  <span className="font-medium">Provedor de LLM:</span>{" "}
                  {llmProvider}
                </div>
                {llmProvider !== "test" && (
                  <>
                    <div>
                      <span className="font-medium">Modelo de geração:</span>{" "}
                      {generationModel}
                    </div>
                    <div>
                      <span className="font-medium">Modelo de embedding:</span>{" "}
                      {embeddingModel}
                    </div>
                  </>
                )}
                <div>
                  <span className="font-medium">Idioma principal:</span>{" "}
                  {DEFAULT_PRIMARY_LANGUAGE}
                </div>
                <div>
                  <span className="font-medium">E-mail do admin:</span>{" "}
                  {adminEmail}
                </div>
              </div>
            </>
          )}
        </div>

        {error && <p className="mt-4 text-sm text-red-600">{error}</p>}

        {/* Navigation */}
        <div className="mt-8 flex justify-between">
          <Button
            variant="secondary"
            onClick={goBack}
            disabled={currentIndex === 0}
          >
            Voltar
          </Button>
          {step === "summary" ? (
            <Button onClick={handleSubmit} disabled={loading}>
              {loading ? (
                <>
                  <Spinner className="mr-2" /> Configurando...
                </>
              ) : (
                "Concluir configuração"
              )}
            </Button>
          ) : (
            <Button onClick={goNext} disabled={!canGoNext()}>
              Próximo
            </Button>
          )}
        </div>
        </div>
      </section>
    </main>
  );
}
