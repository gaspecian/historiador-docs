"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import type { LlmProvider, ProbeResponse } from "@historiador/types";

type Step = "workspace" | "llm" | "languages" | "admin" | "summary";
const STEPS: Step[] = ["workspace", "llm", "languages", "admin", "summary"];

const PROVIDER_OPTIONS = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama (local)" },
  { value: "test", label: "Test (no LLM)" },
];

const COMMON_LANGUAGES = [
  { value: "en", label: "English (en)" },
  { value: "pt-BR", label: "Portuguese - Brazil (pt-BR)" },
  { value: "es", label: "Spanish (es)" },
  { value: "fr", label: "French (fr)" },
  { value: "de", label: "German (de)" },
  { value: "ja", label: "Japanese (ja)" },
  { value: "zh", label: "Chinese (zh)" },
  { value: "ko", label: "Korean (ko)" },
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
  const [primaryLanguage, setPrimaryLanguage] = useState("en");
  const [additionalLanguages, setAdditionalLanguages] = useState<string[]>([]);
  const [adminEmail, setAdminEmail] = useState("");
  const [adminPassword, setAdminPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  const currentIndex = STEPS.indexOf(step);

  const handleProviderChange = (p: LlmProvider) => {
    setLlmProvider(p);
    setProbeResult(null);
    setOllamaModels([]);
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
      case "languages":
        return primaryLanguage.length > 0;
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

    const languages = [
      primaryLanguage,
      ...additionalLanguages.filter((l) => l !== primaryLanguage),
    ];

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
          languages,
          primary_language: primaryLanguage,
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

  const toggleAdditionalLanguage = (lang: string) => {
    setAdditionalLanguages((prev) =>
      prev.includes(lang) ? prev.filter((l) => l !== lang) : [...prev, lang],
    );
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

      <section className="flex flex-col justify-center bg-surface-canvas p-10 md:p-16 max-w-[640px]">
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
              <h2 className="text-lg font-semibold">Workspace name</h2>
              <p className="text-sm text-text-tertiary">
                Choose a name for your documentation workspace.
              </p>
              <Input
                label="Workspace name"
                value={workspaceName}
                onChange={(e) => setWorkspaceName(e.target.value)}
                placeholder="My Docs"
                autoFocus
              />
            </>
          )}

          {/* Step: LLM */}
          {step === "llm" && (
            <>
              <h2 className="text-lg font-semibold">LLM Provider</h2>
              <p className="text-sm text-text-tertiary">
                Select your AI provider for the document editor.
              </p>
              <Select
                label="Provider"
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
                      llmProvider === "ollama" ? "Ollama base URL" : "API Key"
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
                        <Spinner className="mr-2" /> Testing...
                      </>
                    ) : (
                      "Test Connection"
                    )}
                  </Button>
                  {probeResult && (
                    <p
                      className={`text-sm ${probeResult.success ? "text-teal-600" : "text-red-600"}`}
                    >
                      {probeResult.success
                        ? "Connection successful"
                        : probeResult.message}
                    </p>
                  )}

                  {/* Model pickers */}
                  {llmProvider === "ollama" && ollamaModels.length > 0 && (
                    <>
                      <Select
                        label="Generation model"
                        options={ollamaModelOptions}
                        value={generationModel}
                        onChange={(e) => setGenerationModel(e.target.value)}
                      />
                      <Select
                        label="Embedding model"
                        options={ollamaModelOptions}
                        value={embeddingModel}
                        onChange={(e) => setEmbeddingModel(e.target.value)}
                      />
                      <p className="text-xs text-text-tertiary">
                        Need more models? Run{" "}
                        <code>ollama pull &lt;name&gt;</code> and test the
                        connection again.
                      </p>
                    </>
                  )}
                  {llmProvider !== "ollama" && (
                    <>
                      <Input
                        label="Generation model"
                        value={generationModel}
                        onChange={(e) => setGenerationModel(e.target.value)}
                        placeholder={DEFAULT_GEN_MODEL[llmProvider]}
                      />
                      <Input
                        label="Embedding model"
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

          {/* Step: Languages */}
          {step === "languages" && (
            <>
              <h2 className="text-lg font-semibold">Languages</h2>
              <p className="text-sm text-text-tertiary">
                Select the primary language and any additional languages for
                your documentation.
              </p>
              <Select
                label="Primary language"
                options={COMMON_LANGUAGES}
                value={primaryLanguage}
                onChange={(e) => setPrimaryLanguage(e.target.value)}
              />
              <div className="space-y-2">
                <label className="block text-sm font-medium text-text-secondary">
                  Additional languages
                </label>
                <div className="flex flex-wrap gap-2">
                  {COMMON_LANGUAGES.filter(
                    (l) => l.value !== primaryLanguage,
                  ).map((lang) => (
                    <button
                      key={lang.value}
                      type="button"
                      onClick={() => toggleAdditionalLanguage(lang.value)}
                      className={`rounded-full px-3 py-1 text-xs font-medium border transition-colors ${
                        additionalLanguages.includes(lang.value)
                          ? "bg-primary-100 border-primary-200 text-primary-800"
                          : "border-surface-border-strong text-text-secondary hover:bg-surface-subtle"
                      }`}
                    >
                      {lang.label}
                    </button>
                  ))}
                </div>
              </div>
            </>
          )}

          {/* Step: Admin Account */}
          {step === "admin" && (
            <>
              <h2 className="text-lg font-semibold">Admin account</h2>
              <p className="text-sm text-text-tertiary">
                Create the first administrator account.
              </p>
              <Input
                label="Email"
                type="email"
                value={adminEmail}
                onChange={(e) => setAdminEmail(e.target.value)}
                autoComplete="email"
              />
              <Input
                label="Password"
                type="password"
                value={adminPassword}
                onChange={(e) => setAdminPassword(e.target.value)}
                placeholder="Min. 12 characters"
                autoComplete="new-password"
                error={
                  adminPassword.length > 0 && adminPassword.length < 12
                    ? "Min. 12 characters"
                    : undefined
                }
              />
              <Input
                label="Confirm password"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                autoComplete="new-password"
                error={
                  confirmPassword.length > 0 &&
                  confirmPassword !== adminPassword
                    ? "Passwords do not match"
                    : undefined
                }
              />
            </>
          )}

          {/* Step: Summary */}
          {step === "summary" && (
            <>
              <h2 className="text-lg font-semibold">Review & Complete</h2>
              <div className="rounded border border-surface-border p-4 space-y-2 text-sm">
                <div>
                  <span className="font-medium">Workspace:</span>{" "}
                  {workspaceName}
                </div>
                <div>
                  <span className="font-medium">LLM Provider:</span>{" "}
                  {llmProvider}
                </div>
                {llmProvider !== "test" && (
                  <>
                    <div>
                      <span className="font-medium">Generation model:</span>{" "}
                      {generationModel}
                    </div>
                    <div>
                      <span className="font-medium">Embedding model:</span>{" "}
                      {embeddingModel}
                    </div>
                  </>
                )}
                <div>
                  <span className="font-medium">Primary language:</span>{" "}
                  {primaryLanguage}
                </div>
                {additionalLanguages.length > 0 && (
                  <div>
                    <span className="font-medium">Additional:</span>{" "}
                    {additionalLanguages.join(", ")}
                  </div>
                )}
                <div>
                  <span className="font-medium">Admin email:</span> {adminEmail}
                </div>
              </div>
            </>
          )}
        </div>

        {error && <p className="text-sm text-red-600">{error}</p>}

        {/* Navigation */}
        <div className="flex justify-between">
          <Button
            variant="secondary"
            onClick={goBack}
            disabled={currentIndex === 0}
          >
            Back
          </Button>
          {step === "summary" ? (
            <Button onClick={handleSubmit} disabled={loading}>
              {loading ? (
                <>
                  <Spinner className="mr-2" /> Setting up...
                </>
              ) : (
                "Complete Setup"
              )}
            </Button>
          ) : (
            <Button onClick={goNext} disabled={!canGoNext()}>
              Next
            </Button>
          )}
        </div>
      </section>
    </main>
  );
}
