"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { apiStream } from "@/lib/api";

interface Message {
  role: "user" | "assistant";
  content: string;
}

type DeltaEvent = { text: string };
type ErrorEvent = { message: string };
type DoneEvent = { length: number };
type StreamPayload = DeltaEvent | ErrorEvent | DoneEvent;

async function streamMarkdown(
  path: string,
  body: unknown,
  onChunk: (chunk: string) => void,
): Promise<string> {
  let buffer = "";
  for await (const ev of apiStream<StreamPayload>(path, {
    method: "POST",
    body: JSON.stringify(body),
  })) {
    if (ev.event === "delta" && "text" in ev.data) {
      buffer += ev.data.text;
      onChunk(ev.data.text);
    } else if (ev.event === "error" && "message" in ev.data) {
      throw new Error(ev.data.message);
    } else if (ev.event === "done") {
      break;
    }
  }
  return buffer;
}

function Sparkle() {
  return <span aria-hidden className="text-lg">✨</span>;
}

function ArrowRight() {
  return (
    <svg
      width={14}
      height={14}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2.5}
      aria-hidden
    >
      <path d="M5 12h14M13 5l7 7-7 7" />
    </svg>
  );
}

export default function EditorPage() {
  const [brief, setBrief] = useState("");
  const [instruction, setInstruction] = useState("");
  const [draft, setDraft] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [liveAssistant, setLiveAssistant] = useState("");

  const generateDraft = async () => {
    if (!brief.trim() || loading) return;
    setLoading(true);
    setLiveAssistant("");
    setMessages((prev) => [...prev, { role: "user", content: brief }]);

    try {
      const full = await streamMarkdown(
        "/editor/draft",
        { brief },
        (chunk) => setLiveAssistant((prev) => prev + chunk),
      );
      setDraft(full);
      setMessages((prev) => [...prev, { role: "assistant", content: full }]);
      setBrief("");
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${e instanceof Error ? e.message : e}` },
      ]);
    } finally {
      setLoading(false);
      setLiveAssistant("");
    }
  };

  const refineDraft = async () => {
    if (!instruction.trim() || !draft || loading) return;
    setLoading(true);
    setLiveAssistant("");
    setMessages((prev) => [
      ...prev,
      { role: "user", content: `Refine: ${instruction}` },
    ]);

    try {
      const full = await streamMarkdown(
        "/editor/iterate",
        { current_draft: draft, instruction },
        (chunk) => setLiveAssistant((prev) => prev + chunk),
      );
      setDraft(full);
      setMessages((prev) => [...prev, { role: "assistant", content: full }]);
      setInstruction("");
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${e instanceof Error ? e.message : e}` },
      ]);
    } finally {
      setLoading(false);
      setLiveAssistant("");
    }
  };

  const copyToClipboard = () => {
    navigator.clipboard.writeText(draft);
  };

  const showCheckin = draft && !loading;

  return (
    <main className="grid h-full grid-cols-1 md:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] bg-surface-page">
      {/* Conversation pane */}
      <section className="flex flex-col border-r border-surface-border bg-surface-subtle min-h-0">
        <header className="flex h-14 items-center border-b border-surface-border bg-surface-canvas px-6">
          <h1
            className="text-text-primary"
            style={{ fontFamily: "var(--font-display)", fontSize: 20, fontWeight: 400, fontStyle: "italic", margin: 0 }}
          >
            Conversa com a IA
          </h1>
        </header>

        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-3">
          {messages.length === 0 && !loading && (
            <div className="text-sm text-text-tertiary">
              Comece descrevendo o documento que você quer criar. A IA escreve; você guia.
            </div>
          )}

          {messages.map((msg, i) => (
            <div
              key={i}
              className={`rounded-lg px-4 py-3 text-sm ${
                msg.role === "user"
                  ? "bg-surface-canvas border border-surface-border"
                  : "bg-primary-50 text-text-primary"
              }`}
            >
              <div className="mb-1 text-xs font-semibold uppercase text-text-tertiary tracking-wide">
                {msg.role === "user" ? "Você" : "IA"}
              </div>
              <pre className="whitespace-pre-wrap break-words font-sans text-[14px] leading-[1.55]">
                {msg.content}
              </pre>
            </div>
          ))}

          {loading && (
            <>
              {liveAssistant ? (
                <div className="rounded-lg px-4 py-3 text-sm bg-primary-50 text-text-primary">
                  <div className="mb-1 text-xs font-semibold uppercase text-text-tertiary tracking-wide">
                    IA
                  </div>
                  <pre className="whitespace-pre-wrap break-words font-sans text-[14px] leading-[1.55]">
                    {liveAssistant}
                    <span className="animate-pulse">▍</span>
                  </pre>
                </div>
              ) : (
                <div className="inline-flex items-center gap-2 rounded-full bg-primary-50 px-3 py-1 text-xs font-semibold text-primary-700">
                  <span className="relative inline-block h-2 w-2 rounded-full bg-primary-600">
                    <span
                      className="absolute rounded-full border-2 border-primary-600 opacity-35"
                      style={{ inset: -3, animation: "pulse 1.6s infinite" }}
                    />
                  </span>
                  Escrevendo…
                </div>
              )}
            </>
          )}
        </div>

        <div className="border-t border-surface-border bg-surface-canvas p-4">
          {!draft ? (
            <div className="space-y-2">
              <textarea
                className="w-full rounded-md border border-surface-border bg-surface-canvas p-3 text-sm text-text-primary placeholder:text-text-disabled focus:border-primary-600 focus:outline-none focus-visible:[box-shadow:var(--shadow-focus)]"
                rows={3}
                placeholder="Descreva o documento que você quer criar…"
                value={brief}
                onChange={(e) => setBrief(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && e.metaKey) generateDraft();
                }}
              />
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-text-tertiary" style={{ fontFamily: "var(--font-mono)" }}>
                  ⌘+Enter para gerar
                </span>
                <Button onClick={generateDraft} disabled={loading || !brief.trim()}>
                  Gerar rascunho
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex gap-2">
              <input
                className="flex-1 h-10 rounded-md border border-surface-border bg-surface-canvas px-3 text-sm text-text-primary placeholder:text-text-disabled focus:border-primary-600 focus:outline-none focus-visible:[box-shadow:var(--shadow-focus)]"
                placeholder="Descreva o que mudar…"
                value={instruction}
                onChange={(e) => setInstruction(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") refineDraft();
                }}
              />
              <Button onClick={refineDraft} disabled={loading || !instruction.trim()}>
                Refinar
              </Button>
            </div>
          )}
        </div>
      </section>

      {/* Document pane */}
      <section className="flex flex-col min-h-0 bg-surface-page">
        <header className="flex h-14 items-center justify-between bg-surface-canvas border-b border-surface-border px-6 gap-4">
          <div className="text-[13px] text-text-secondary">
            {draft ? "Rascunho" : "Sem documento ainda"}
          </div>
          {draft && (
            <Button variant="secondary" size="sm" onClick={copyToClipboard}>
              Copiar markdown
            </Button>
          )}
        </header>

        <div className="flex-1 overflow-y-auto px-10 py-8">
          <div className="mx-auto" style={{ maxWidth: "var(--content-max)" }}>
            {showCheckin && (
              <div
                className="relative mb-6 rounded-lg border border-primary-100 bg-surface-canvas p-5 shadow-sm"
              >
                <span
                  className="pointer-events-none absolute inset-0 rounded-lg"
                  style={{ boxShadow: "0 0 0 3px var(--color-primary-50)" }}
                />
                <div className="mb-3 flex items-center gap-2">
                  <Sparkle />
                  <span className="text-sm font-semibold text-text-primary">
                    Rascunho pronto — ficou como você imaginou?
                  </span>
                </div>
                <div className="mb-3.5 text-[13px] leading-relaxed text-text-secondary">
                  Você pode continuar refinando ou copiar o markdown. Suas alterações ficam no
                  rascunho até você salvar.
                </div>
                <div className="flex gap-2">
                  <Button size="sm">
                    <span className="inline-flex items-center gap-1.5">
                      Continuar escrevendo
                      <ArrowRight />
                    </span>
                  </Button>
                  <Button size="sm" variant="secondary" onClick={copyToClipboard}>
                    Copiar markdown
                  </Button>
                </div>
              </div>
            )}

            {draft ? (
              <pre className="whitespace-pre-wrap break-words text-[15px] leading-[1.6] text-text-primary font-sans">
                {draft}
              </pre>
            ) : (
              <div
                className="text-text-tertiary"
                style={{ fontFamily: "var(--font-display)", fontSize: 28, fontStyle: "italic", lineHeight: 1.3 }}
              >
                O documento aparece aqui quando a conversa começar.
              </div>
            )}
          </div>
        </div>
      </section>
    </main>
  );
}
