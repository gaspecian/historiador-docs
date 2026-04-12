"use client";

import { useState } from "react";
import { apiFetch } from "@/lib/api";
import { Button } from "@/components/ui/button";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface Props {
  initialContent?: string;
  language?: string;
  onSave?: (markdown: string) => void;
}

export function EditorPanel({ initialContent, language, onSave }: Props) {
  const [brief, setBrief] = useState("");
  const [instruction, setInstruction] = useState("");
  const [draft, setDraft] = useState(initialContent || "");
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);

  const generateDraft = async () => {
    if (!brief.trim() || loading) return;
    setLoading(true);
    setMessages((prev) => [...prev, { role: "user", content: brief }]);

    try {
      const data = await apiFetch<{ content_markdown: string }>("/editor/draft", {
        method: "POST",
        body: JSON.stringify({ brief, language }),
      });
      setDraft(data.content_markdown);
      setMessages((prev) => [...prev, { role: "assistant", content: data.content_markdown }]);
      setBrief("");
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${e instanceof Error ? e.message : e}` },
      ]);
    } finally {
      setLoading(false);
    }
  };

  const refineDraft = async () => {
    if (!instruction.trim() || !draft || loading) return;
    setLoading(true);
    setMessages((prev) => [...prev, { role: "user", content: `Refine: ${instruction}` }]);

    try {
      const data = await apiFetch<{ content_markdown: string }>("/editor/iterate", {
        method: "POST",
        body: JSON.stringify({ current_draft: draft, instruction }),
      });
      setDraft(data.content_markdown);
      setMessages((prev) => [...prev, { role: "assistant", content: data.content_markdown }]);
      setInstruction("");
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${e instanceof Error ? e.message : e}` },
      ]);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Message history */}
      {messages.length > 0 && (
        <div className="max-h-64 overflow-y-auto space-y-3">
          {messages.map((msg, i) => (
            <div
              key={i}
              className={`p-3 rounded text-sm ${
                msg.role === "user"
                  ? "bg-blue-50 dark:bg-blue-950"
                  : "bg-zinc-100 dark:bg-zinc-900"
              }`}
            >
              <div className="text-xs text-zinc-500 mb-1">
                {msg.role === "user" ? "You" : "AI"}
              </div>
              <pre className="whitespace-pre-wrap break-words font-mono text-xs">
                {msg.content}
              </pre>
            </div>
          ))}
          {loading && (
            <div className="p-3 rounded bg-zinc-100 dark:bg-zinc-900 text-zinc-500 text-sm">
              Generating...
            </div>
          )}
        </div>
      )}

      {/* Input area */}
      {!draft ? (
        <div className="space-y-2">
          <textarea
            className="w-full p-3 border rounded text-sm bg-white dark:bg-zinc-800 dark:border-zinc-700"
            rows={3}
            placeholder="Describe the document you want to create..."
            value={brief}
            onChange={(e) => setBrief(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && e.metaKey) generateDraft();
            }}
          />
          <Button onClick={generateDraft} disabled={loading || !brief.trim()}>
            Generate Draft
          </Button>
        </div>
      ) : (
        <div className="space-y-2">
          <div className="flex gap-2">
            <input
              className="flex-1 p-2 border rounded text-sm bg-white dark:bg-zinc-800 dark:border-zinc-700"
              placeholder="Describe what to change..."
              value={instruction}
              onChange={(e) => setInstruction(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") refineDraft();
              }}
            />
            <Button onClick={refineDraft} disabled={loading || !instruction.trim()}>
              Refine
            </Button>
          </div>
          <div className="flex gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => navigator.clipboard.writeText(draft)}
            >
              Copy to clipboard
            </Button>
            {onSave && (
              <Button size="sm" onClick={() => onSave(draft)}>
                Save to page
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
