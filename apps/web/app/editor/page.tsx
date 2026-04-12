"use client";

import { useState } from "react";

interface Message {
  role: "user" | "assistant";
  content: string;
}

export default function EditorPage() {
  const [brief, setBrief] = useState("");
  const [instruction, setInstruction] = useState("");
  const [draft, setDraft] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);

  // Read token from localStorage (set after login).
  const getToken = () =>
    typeof window !== "undefined" ? localStorage.getItem("access_token") ?? "" : "";

  const generateDraft = async () => {
    if (!brief.trim() || loading) return;
    setLoading(true);
    setMessages((prev) => [...prev, { role: "user", content: brief }]);

    try {
      const res = await fetch("/api/editor/draft", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${getToken()}`,
        },
        body: JSON.stringify({ brief }),
      });

      if (!res.ok) {
        const err = await res.text();
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: `Error ${res.status}: ${err}` },
        ]);
        return;
      }

      const data = await res.json();
      setDraft(data.content_markdown);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: data.content_markdown },
      ]);
      setBrief("");
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Network error: ${e}` },
      ]);
    } finally {
      setLoading(false);
    }
  };

  const refineDraft = async () => {
    if (!instruction.trim() || !draft || loading) return;
    setLoading(true);
    setMessages((prev) => [
      ...prev,
      { role: "user", content: `Refine: ${instruction}` },
    ]);

    try {
      const res = await fetch("/api/editor/iterate", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${getToken()}`,
        },
        body: JSON.stringify({
          current_draft: draft,
          instruction,
        }),
      });

      if (!res.ok) {
        const err = await res.text();
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: `Error ${res.status}: ${err}` },
        ]);
        return;
      }

      const data = await res.json();
      setDraft(data.content_markdown);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: data.content_markdown },
      ]);
      setInstruction("");
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Network error: ${e}` },
      ]);
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = () => {
    navigator.clipboard.writeText(draft);
  };

  return (
    <main className="p-10 font-mono text-sm max-w-4xl mx-auto">
      <h1 className="text-xl font-bold mb-6">AI Document Editor</h1>

      {/* Message history */}
      <div className="mb-6 max-h-96 overflow-y-auto space-y-4">
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`p-3 rounded ${
              msg.role === "user"
                ? "bg-blue-50 dark:bg-blue-950"
                : "bg-zinc-100 dark:bg-zinc-900"
            }`}
          >
            <div className="text-xs text-zinc-500 mb-1">
              {msg.role === "user" ? "You" : "AI"}
            </div>
            <pre className="whitespace-pre-wrap break-words">
              {msg.content}
            </pre>
          </div>
        ))}
        {loading && (
          <div className="p-3 rounded bg-zinc-100 dark:bg-zinc-900 text-zinc-500">
            Generating...
          </div>
        )}
      </div>

      {/* Input area */}
      {!draft ? (
        <div className="space-y-3">
          <textarea
            className="w-full p-3 border rounded bg-white dark:bg-zinc-800 dark:border-zinc-700"
            rows={4}
            placeholder="Describe the document you want to create..."
            value={brief}
            onChange={(e) => setBrief(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && e.metaKey) generateDraft();
            }}
          />
          <button
            className="px-4 py-2 bg-blue-600 text-white rounded disabled:opacity-50"
            onClick={generateDraft}
            disabled={loading || !brief.trim()}
          >
            Generate Draft
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          <div className="flex gap-2">
            <input
              className="flex-1 p-3 border rounded bg-white dark:bg-zinc-800 dark:border-zinc-700"
              placeholder="Describe what to change..."
              value={instruction}
              onChange={(e) => setInstruction(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") refineDraft();
              }}
            />
            <button
              className="px-4 py-2 bg-blue-600 text-white rounded disabled:opacity-50"
              onClick={refineDraft}
              disabled={loading || !instruction.trim()}
            >
              Refine
            </button>
          </div>
          <button
            className="px-4 py-2 border rounded text-zinc-600 dark:text-zinc-400"
            onClick={copyToClipboard}
          >
            Copy draft to clipboard
          </button>
        </div>
      )}
    </main>
  );
}
