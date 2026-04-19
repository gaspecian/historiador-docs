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
        // The conversation pane is for thinking, not for shipping the
        // artefact. The generated markdown lands on the canvas; chat
        // gets a short status so the timeline reads cleanly.
        setDraft(full);
        setMessages((prev) => [
          ...prev,
          {
            role: "assistant",
            content: "Rascunho atualizado — veja o canvas à direita.",
          },
        ]);
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

  return {
    draft,
    messages,
    streaming,
    liveAssistant,
    generateDraft,
    iterateDraft,
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
