// Editor service — SSE-streamed draft + iterate operations. Callers
// consume these as async generators and drive their own UI state.

import { apiStream, type ApiStreamEvent } from "../api";
import type { DraftRequest, IterateRequest } from "@historiador/types";

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
