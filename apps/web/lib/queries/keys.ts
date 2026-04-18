// Centralized query keys so invalidation stays consistent across
// files. Each bounded context exposes its keys under a namespace:
//
//   queryKeys.pages.list(collectionId)
//   queryKeys.pages.detail(id)
//
// Mutations invalidate with the parent namespace
// (e.g. queryKeys.pages.all) to catch every dependent query at once.

export const queryKeys = {
  pages: {
    all: ["pages"] as const,
    list: (collectionId: string | null) =>
      ["pages", "list", collectionId ?? "root"] as const,
    search: (query: string) => ["pages", "search", query] as const,
    detail: (id: string) => ["pages", "detail", id] as const,
    versions: (id: string) => ["pages", "versions", id] as const,
    history: (pageId: string, language: string, page: number, perPage: number) =>
      ["pages", "history", pageId, language, page, perPage] as const,
    historyItem: (pageId: string, historyId: string) =>
      ["pages", "history-item", pageId, historyId] as const,
  },
  collections: {
    all: ["collections"] as const,
    list: () => ["collections", "list"] as const,
  },
  admin: {
    all: ["admin"] as const,
    users: () => ["admin", "users"] as const,
    workspace: () => ["admin", "workspace"] as const,
    mcpAnalytics: (days: number) => ["admin", "analytics", "mcp", days] as const,
  },
} as const;
