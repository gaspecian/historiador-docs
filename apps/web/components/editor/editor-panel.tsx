"use client";

import { useState } from "react";
import { useEditorStream } from "@/features/editor";
import { Button } from "@/components/ui/button";

interface Props {
 initialContent?: string;
 language?: string;
 onSave?: (markdown: string) => void;
}

export function EditorPanel({ initialContent, language, onSave }: Props) {
 const [brief, setBrief] = useState("");
 const [instruction, setInstruction] = useState("");
 const { draft, messages, streaming, liveAssistant, generateDraft, iterateDraft } =
 useEditorStream({ initialDraft: initialContent });

 const submitGenerate = async () => {
 if (!brief.trim() || streaming) return;
 await generateDraft({ brief, language });
 setBrief("");
 };

 const submitRefine = async () => {
 if (!instruction.trim() || !draft || streaming) return;
 await iterateDraft({ instruction });
 setInstruction("");
 };

 return (
 <div className="space-y-4">
 {/* Message history */}
 {(messages.length > 0 || streaming) && (
 <div className="max-h-64 overflow-y-auto space-y-3">
 {messages.map((msg, i) => (
 <div
 key={i}
 className={`p-3 rounded text-sm ${
 msg.role === "user"
 ? "bg-primary-50"
 : "bg-surface-hover"
 }`}
 >
 <div className="text-xs text-text-tertiary mb-1">
 {msg.role === "user" ? "You" : "AI"}
 </div>
 <pre className="whitespace-pre-wrap break-words font-mono text-xs">
 {msg.content}
 </pre>
 </div>
 ))}
 {streaming && (
 <div className="p-3 rounded bg-surface-hover text-sm">
 <div className="text-xs text-text-tertiary mb-1">AI</div>
 {liveAssistant ? (
 <pre className="whitespace-pre-wrap break-words font-mono text-xs">
 {liveAssistant}
 <span className="animate-pulse">▍</span>
 </pre>
 ) : (
 <span className="text-text-tertiary">Generating…</span>
 )}
 </div>
 )}
 </div>
 )}

 {/* Input area */}
 {!draft ? (
 <div className="space-y-2">
 <textarea
 className="w-full p-3 border rounded text-sm bg-white"
 rows={3}
 placeholder="Describe the document you want to create..."
 value={brief}
 onChange={(e) => setBrief(e.target.value)}
 onKeyDown={(e) => {
 if (e.key === "Enter" && e.metaKey) submitGenerate();
 }}
 />
 <Button onClick={submitGenerate} disabled={streaming || !brief.trim()}>
 Generate Draft
 </Button>
 </div>
 ) : (
 <div className="space-y-2">
 <div className="flex gap-2">
 <input
 className="flex-1 p-2 border rounded text-sm bg-white"
 placeholder="Describe what to change..."
 value={instruction}
 onChange={(e) => setInstruction(e.target.value)}
 onKeyDown={(e) => {
 if (e.key === "Enter") submitRefine();
 }}
 />
 <Button onClick={submitRefine} disabled={streaming || !instruction.trim()}>
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
