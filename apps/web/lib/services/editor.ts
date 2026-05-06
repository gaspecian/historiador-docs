// Editor service — SSE-streamed draft + iterate operations plus the
// conversation-persistence endpoints added in Sprint 10. Stream helpers
// return async generators; the persistence helpers are plain requests.

import { apiFetch, apiStream, type ApiStreamEvent } from "../api";
import type {
  ConversationMessageDto,
  ConversationResponse,
  DraftRequest,
  IterateRequest,
  SaveConversationRequest,
} from "@historiador/types";

export type EditorDelta = { text: string };
export type EditorError = { message: string };
export type EditorDone = { length: number };
export type EditorEvent =
  | ApiStreamEvent<EditorDelta>
  | ApiStreamEvent<EditorError>
  | ApiStreamEvent<EditorDone>;

export function draft(body: DraftRequest): AsyncGenerator<EditorEvent, void, void> {
  return apiStream<EditorDelta | EditorError | EditorDone>("/editor/draft", {
    method: "POST",
    body: JSON.stringify(body),
  }) as AsyncGenerator<EditorEvent, void, void>;
}

export function iterate(body: IterateRequest): AsyncGenerator<EditorEvent, void, void> {
  return apiStream<EditorDelta | EditorError | EditorDone>("/editor/iterate", {
    method: "POST",
    body: JSON.stringify(body),
  }) as AsyncGenerator<EditorEvent, void, void>;
}

// ---- conversation persistence (Sprint 10 item #4) ----

function conversationPath(pageId: string, language: string): string {
  const qs = new URLSearchParams({ language }).toString();
  return `/pages/${encodeURIComponent(pageId)}/editor-conversation?${qs}`;
}

export async function loadConversation(
  pageId: string,
  language: string,
): Promise<ConversationResponse> {
  return apiFetch<ConversationResponse>(conversationPath(pageId, language));
}

export async function saveConversation(
  pageId: string,
  language: string,
  messages: ConversationMessageDto[],
): Promise<ConversationResponse> {
  const body: SaveConversationRequest = { messages };
  return apiFetch<ConversationResponse>(conversationPath(pageId, language), {
    method: "PUT",
    body: JSON.stringify(body),
  });
}
