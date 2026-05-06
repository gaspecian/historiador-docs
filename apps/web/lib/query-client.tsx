"use client";

// TanStack Query client + provider. Instantiated lazily via useState
// so the QueryClient survives re-renders but is **not** shared across
// SSR requests (critical for Next.js: a module-level singleton would
// leak cache between users). Per the TanStack + Next.js guide.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        // The API is behind the Next rewrite and round-trips are
        // cheap; prefer freshness over aggressive caching. Feature
        // hooks override per-query when they need something more
        // specialized.
        staleTime: 10_000,
        // Retrying 401 is pointless — apiFetch already transparently
        // refreshes or redirects, so if a query still fails it's a
        // real failure.
        retry: 1,
        refetchOnWindowFocus: false,
      },
      mutations: {
        retry: 0,
      },
    },
  });
}

export function QueryProviders({ children }: { children: ReactNode }) {
  const [client] = useState(makeQueryClient);
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
