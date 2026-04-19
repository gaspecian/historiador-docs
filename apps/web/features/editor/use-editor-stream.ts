"use client";

// Shared SSE stream driver for the AI editor. Wraps an editor service
// call (draft / iterate / etc.) plus the small state machine every
// editor UI needs: in-flight flag, live-assistant buffer, message
// history, final-draft string, error on completion.

import { useCallback, useState } from "react";
import * as editorService from "@/lib/services/editor";
import type { EditorEvent } from "@/lib/services/editor";
import type {
  DraftRequest,
  IterateRequest,
} from "@historiador/types";

export interface EditorMessage {
  role: "user" | "assistant";
  content: string;
}

export interface UseEditorStreamOptions {
  /** Seeds the internal draft buffer when the caller already has one. */
  initialDraft?: string;
}

export interface UseEditorStreamResult {
  /** Accumulated markdown across all generate/iterate turns. */
  draft: string;
  /** Chat-style transcript of user briefs / assistant completions. */
  messages: EditorMessage[];
  /** True while an upstream stream is still yielding events. */
  streaming: boolean;
  /** The partial assistant response as it arrives. Cleared on done. */
  liveAssistant: string;

  /** Kick off an /editor/draft stream. */
  generateDraft: (body: DraftRequest) => Promise<void>;
  /** Kick off an /editor/iterate stream using the current draft. */
  iterateDraft: (body: Omit<IterateRequest, "current_draft">) => Promise<void>;
  /** Compose a block-level comment into an iterate instruction and
   *  send it to the AI. The agent reads the current draft + the
   *  comment context and decides whether to update the canvas or
   *  just answer in chat. */
  submitBlockComment: (blockSource: string, commentText: string) => Promise<void>;
  /** Overwrite the draft (e.g. restore from server, bail out). */
  setDraft: (value: string) => void;
}

export function useEditorStream(
  opts: UseEditorStreamOptions = {},
): UseEditorStreamResult {
  const [draft, setDraft] = useState(opts.initialDraft ?? "");
  const [messages, setMessages] = useState<EditorMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [liveAssistant, setLiveAssistant] = useState("");

  const runStream = useCallback(
    async (
      userPreview: string,
      stream: AsyncGenerator<EditorEvent, void, void>,
    ) => {
      setStreaming(true);
      setLiveAssistant("");
      setMessages((prev) => [...prev, { role: "user", content: userPreview }]);

      try {
        const full = await collectStream(stream, (chunk) => {
          setLiveAssistant((prev) => prev + chunk);
        });
        // The agent prompt splits its reply into <chat>/<canvas>
        // tags; parse them so conversation goes to the messages
        // list and document content goes to the draft pane. A
        // pure-conversation turn carries no <canvas>, so the
        // existing draft stays put.
        const { chat, canvas } = splitChannels(full);
        if (canvas.length > 0) {
          setDraft(canvas);
        }
        const chatContent =
          chat.length > 0
            ? chat
            : canvas.length > 0
              ? "Rascunho atualizado — veja o canvas à direita."
              : full.trim();
        if (chatContent.length > 0) {
          setMessages((prev) => [...prev, { role: "assistant", content: chatContent }]);
        }
      } catch (e) {
        setMessages((prev) => [
          ...prev,
          {
            role: "assistant",
            content: `Error: ${e instanceof Error ? e.message : String(e)}`,
          },
        ]);
      } finally {
        setStreaming(false);
        setLiveAssistant("");
      }
    },
    [],
  );

  const generateDraft = useCallback(
    async (body: DraftRequest) => {
      if (!body.brief.trim() || streaming) return;
      await runStream(body.brief, editorService.draft(body));
    },
    [runStream, streaming],
  );

  const iterateDraft = useCallback(
    async (body: Omit<IterateRequest, "current_draft">) => {
      if (!body.instruction.trim() || !draft || streaming) return;
      await runStream(
        `Refine: ${body.instruction}`,
        editorService.iterate({ ...body, current_draft: draft }),
      );
    },
    [runStream, draft, streaming],
  );

  const submitBlockComment = useCallback(
    async (blockSource: string, commentText: string) => {
      if (!commentText.trim() || !draft || streaming) return;
      // Compose a GitHub-PR-style instruction: quote the targeted
      // block so the agent knows exactly where the comment is, then
      // ask it to either update the doc or reply in chat (the
      // channel-tag contract gives it both options).
      const snippet = blockSource.trim().slice(0, 240);
      const instruction =
        `O usuário comentou no seguinte trecho do documento:\n\n` +
        `>>>\n${snippet}\n<<<\n\n` +
        `Comentário: "${commentText.trim()}"\n\n` +
        `Avalie o comentário. Se fizer sentido, ` +
        `atualize o documento inteiro em <canvas>. ` +
        `Responda em <chat> explicando brevemente o que você mudou ` +
        `ou por que não fez sentido mudar.`;
      await runStream(
        `Comentário: ${commentText}`,
        editorService.iterate({ instruction, current_draft: draft }),
      );
    },
    [runStream, draft, streaming],
  );

  return {
    draft,
    messages,
    streaming,
    liveAssistant,
    generateDraft,
    iterateDraft,
    submitBlockComment,
    setDraft,
  };
}

/**
 * Consume an `EditorEvent` stream. Appends delta text to the caller
 * via `onChunk`, surfaces error events by throwing, and returns the
 * full buffer once the `done` event arrives. Exposed for the rare
 * component that wants to drive the stream itself without the
 * surrounding state machine.
 */
export async function collectStream(
  stream: AsyncGenerator<EditorEvent, void, void>,
  onChunk: (chunk: string) => void,
): Promise<string> {
  let buffer = "";
  for await (const ev of stream) {
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

/**
 * Mirrors `apps/api/src/application/editor/channels.rs`. Forgiving
 * parser: case-insensitive tags, tolerates attributes on the open
 * tag, handles arbitrary content (backticks, angle brackets) inside
 * the tag body. If neither tag appears, the whole reply falls into
 * the chat channel so content never gets silently dropped.
 */
function splitChannels(raw: string): { chat: string; canvas: string } {
  const chat = extractTag(raw, "chat");
  const canvas = extractTag(raw, "canvas");
  if (chat === null && canvas === null) {
    // Fallback: strip orphan tag markers so the user never sees raw
    // protocol fragments when the agent violates the contract.
    return { chat: stripOrphanTags(raw).trim(), canvas: "" };
  }
  return { chat: (chat ?? "").trim(), canvas: (canvas ?? "").trim() };
}

function stripOrphanTags(raw: string): string {
  return raw.replace(/<\/?(chat|canvas)\s*>/gi, "");
}

function extractTag(raw: string, tag: string): string | null {
  const lower = raw.toLowerCase();
  const open = `<${tag}`;
  const close = `</${tag}>`;
  const openPos = lower.indexOf(open);
  if (openPos < 0) return null;
  // Skip past the `>` that closes the opening tag so attributes work.
  const gt = raw.indexOf(">", openPos);
  if (gt < 0) return null;
  const closePos = lower.indexOf(close, gt + 1);
  if (closePos < 0) return null;
  return raw.slice(gt + 1, closePos);
}
