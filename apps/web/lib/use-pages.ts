"use client";

import { useCallback, useEffect, useState } from "react";
import * as pagesService from "./services/pages";
import type { PageResponse } from "@historiador/types";

export function usePages(collectionId: string | null) {
  const [pages, setPages] = useState<PageResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPages = useCallback(async () => {
    try {
      setIsLoading(true);
      const data = await pagesService.list(collectionId);
      setPages(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load pages");
    } finally {
      setIsLoading(false);
    }
  }, [collectionId]);

  useEffect(() => {
    fetchPages();
  }, [fetchPages]);

  const search = useCallback(async (query: string) => {
    if (!query.trim()) {
      fetchPages();
      return;
    }
    try {
      setIsLoading(true);
      const data = await pagesService.search(query);
      setPages(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setIsLoading(false);
    }
  }, [fetchPages]);

  return {
    pages,
    isLoading,
    error,
    refresh: fetchPages,
    search,
  };
}
