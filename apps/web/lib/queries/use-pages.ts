"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as pagesService from "@/lib/services/pages";
import { queryKeys } from "./keys";
import type {
  CreatePageRequest,
  UpdatePageRequest,
} from "@historiador/types";

export function usePagesQuery(collectionId: string | null) {
  return useQuery({
    queryKey: queryKeys.pages.list(collectionId),
    queryFn: () => pagesService.list(collectionId),
  });
}

export function useSearchPagesQuery(query: string, enabled = true) {
  return useQuery({
    queryKey: queryKeys.pages.search(query),
    queryFn: () => pagesService.search(query),
    enabled: enabled && query.trim().length > 0,
  });
}

export function usePageQuery(id: string | null) {
  return useQuery({
    queryKey: id ? queryKeys.pages.detail(id) : ["pages", "detail", "noop"],
    queryFn: () => pagesService.get(id as string),
    enabled: !!id,
  });
}

export function usePageVersionsQuery(id: string | null) {
  return useQuery({
    queryKey: id ? queryKeys.pages.versions(id) : ["pages", "versions", "noop"],
    queryFn: () => pagesService.versions(id as string),
    enabled: !!id,
  });
}

export function useVersionHistoryQuery(
  pageId: string | null,
  language: string,
  page: number,
  perPage: number,
) {
  return useQuery({
    queryKey: pageId
      ? queryKeys.pages.history(pageId, language, page, perPage)
      : ["pages", "history", "noop"],
    queryFn: () =>
      pagesService.history(pageId as string, { language, page, per_page: perPage }),
    enabled: !!pageId,
  });
}

export function useVersionHistoryItemQuery(
  pageId: string | null,
  historyId: string | null,
) {
  return useQuery({
    queryKey:
      pageId && historyId
        ? queryKeys.pages.historyItem(pageId, historyId)
        : ["pages", "history-item", "noop"],
    queryFn: () => pagesService.historyItem(pageId as string, historyId as string),
    enabled: !!pageId && !!historyId,
  });
}

// ---- mutations ----

export function useCreatePageMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreatePageRequest) => pagesService.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.pages.all });
    },
  });
}

export function useUpdatePageMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdatePageRequest }) =>
      pagesService.update(id, body),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: queryKeys.pages.detail(vars.id) });
      qc.invalidateQueries({ queryKey: queryKeys.pages.all });
    },
  });
}

export function usePublishPageMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => pagesService.publish(id),
    onSuccess: (_data, id) => {
      qc.invalidateQueries({ queryKey: queryKeys.pages.detail(id) });
      qc.invalidateQueries({ queryKey: queryKeys.pages.all });
    },
  });
}

export function useDraftPageMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => pagesService.draft(id),
    onSuccess: (_data, id) => {
      qc.invalidateQueries({ queryKey: queryKeys.pages.detail(id) });
      qc.invalidateQueries({ queryKey: queryKeys.pages.all });
    },
  });
}

export function useRestoreVersionMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ pageId, historyId }: { pageId: string; historyId: string }) =>
      pagesService.restoreVersion(pageId, historyId),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: queryKeys.pages.detail(vars.pageId) });
      qc.invalidateQueries({ queryKey: queryKeys.pages.all });
    },
  });
}
