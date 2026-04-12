"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import type { LlmProvider, ProbeResponse, SetupResponse } from "@/lib/types";

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
  const [primaryLanguage, setPrimaryLanguage] = useState("en");
  const [additionalLanguages, setAdditionalLanguages] = useState<string[]>([]);
  const [adminEmail, setAdminEmail] = useState("");
  const [adminPassword, setAdminPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  const currentIndex = STEPS.indexOf(step);

  const canGoNext = (): boolean => {
    switch (step) {
      case "workspace":
        return workspaceName.trim().length > 0;
      case "llm":
        return llmProvider === "test" || llmApiKey.trim().length > 0;
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
    try {
      const res = await fetch("/api/setup/probe", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ llm_provider: llmProvider, llm_api_key: llmApiKey }),
      });
      const data: ProbeResponse = await res.json();
      setProbeResult(data);
    } catch (err) {
      setProbeResult({ success: false, message: err instanceof Error ? err.message : "Connection failed" });
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async () => {
    setError("");
    setLoading(true);

    const languages = [primaryLanguage, ...additionalLanguages.filter((l) => l !== primaryLanguage)];

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
          languages,
          primary_language: primaryLanguage,
        }),
      });

      if (!res.ok) {
        const body = await res.json();
        throw new Error(body.message || "Setup failed");
      }

      const _data: SetupResponse = await res.json();

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

  return (
    <main className="flex min-h-screen items-center justify-center p-4">
      <div className="w-full max-w-lg space-y-6">
        <div className="text-center">
          <h1 className="text-2xl font-bold">Historiador Doc Setup</h1>
          <p className="mt-1 text-sm text-zinc-500">
            Step {currentIndex + 1} of {STEPS.length}
          </p>
          {/* Progress bar */}
          <div className="mt-3 flex gap-1">
            {STEPS.map((s, i) => (
              <div
                key={s}
                className={`h-1 flex-1 rounded ${i <= currentIndex ? "bg-blue-600" : "bg-zinc-200 dark:bg-zinc-700"}`}
              />
            ))}
          </div>
        </div>

        <div className="space-y-4">
          {/* Step: Workspace */}
          {step === "workspace" && (
            <>
              <h2 className="text-lg font-semibold">Workspace name</h2>
              <p className="text-sm text-zinc-500">
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
              <p className="text-sm text-zinc-500">
                Select your AI provider for the document editor.
              </p>
              <Select
                label="Provider"
                options={PROVIDER_OPTIONS}
                value={llmProvider}
                onChange={(e) => {
                  setLlmProvider(e.target.value as LlmProvider);
                  setProbeResult(null);
                }}
              />
              {llmProvider !== "test" && (
                <>
                  <Input
                    label={llmProvider === "ollama" ? "Ollama base URL" : "API Key"}
                    type={llmProvider === "ollama" ? "url" : "password"}
                    value={llmApiKey}
                    onChange={(e) => {
                      setLlmApiKey(e.target.value);
                      setProbeResult(null);
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
                    {loading ? <><Spinner className="mr-2" /> Testing...</> : "Test Connection"}
                  </Button>
                  {probeResult && (
                    <p className={`text-sm ${probeResult.success ? "text-green-600" : "text-red-600"}`}>
                      {probeResult.success ? "Connection successful" : probeResult.message}
                    </p>
                  )}
                </>
              )}
            </>
          )}

          {/* Step: Languages */}
          {step === "languages" && (
            <>
              <h2 className="text-lg font-semibold">Languages</h2>
              <p className="text-sm text-zinc-500">
                Select the primary language and any additional languages for your documentation.
              </p>
              <Select
                label="Primary language"
                options={COMMON_LANGUAGES}
                value={primaryLanguage}
                onChange={(e) => setPrimaryLanguage(e.target.value)}
              />
              <div className="space-y-2">
                <label className="block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  Additional languages
                </label>
                <div className="flex flex-wrap gap-2">
                  {COMMON_LANGUAGES.filter((l) => l.value !== primaryLanguage).map((lang) => (
                    <button
                      key={lang.value}
                      type="button"
                      onClick={() => toggleAdditionalLanguage(lang.value)}
                      className={`rounded-full px-3 py-1 text-xs font-medium border transition-colors ${
                        additionalLanguages.includes(lang.value)
                          ? "bg-blue-100 border-blue-300 text-blue-800 dark:bg-blue-900 dark:border-blue-700 dark:text-blue-200"
                          : "border-zinc-300 dark:border-zinc-600 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-50 dark:hover:bg-zinc-800"
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
              <p className="text-sm text-zinc-500">
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
                error={adminPassword.length > 0 && adminPassword.length < 12 ? "Min. 12 characters" : undefined}
              />
              <Input
                label="Confirm password"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                autoComplete="new-password"
                error={confirmPassword.length > 0 && confirmPassword !== adminPassword ? "Passwords do not match" : undefined}
              />
            </>
          )}

          {/* Step: Summary */}
          {step === "summary" && (
            <>
              <h2 className="text-lg font-semibold">Review & Complete</h2>
              <div className="rounded border border-zinc-200 dark:border-zinc-700 p-4 space-y-2 text-sm">
                <div><span className="font-medium">Workspace:</span> {workspaceName}</div>
                <div><span className="font-medium">LLM Provider:</span> {llmProvider}</div>
                <div><span className="font-medium">Primary language:</span> {primaryLanguage}</div>
                {additionalLanguages.length > 0 && (
                  <div><span className="font-medium">Additional:</span> {additionalLanguages.join(", ")}</div>
                )}
                <div><span className="font-medium">Admin email:</span> {adminEmail}</div>
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
              {loading ? <><Spinner className="mr-2" /> Setting up...</> : "Complete Setup"}
            </Button>
          ) : (
            <Button onClick={goNext} disabled={!canGoNext()}>
              Next
            </Button>
          )}
        </div>
      </div>
    </main>
  );
}
