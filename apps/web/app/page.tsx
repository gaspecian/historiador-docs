"use client";

import { useEffect, useState } from "react";

export default function Home() {
  const [health, setHealth] = useState<string>("loading\u2026");

  useEffect(() => {
    fetch("/api/health")
      .then((r) => r.json())
      .then((j) => setHealth(JSON.stringify(j, null, 2)))
      .catch((e) => setHealth(`error: ${e.message}`));
  }, []);

  return (
    <main className="p-10 font-mono text-sm">
      <h1 className="text-xl font-bold mb-4">
        Historiador Doc — Sprint 1 check
      </h1>
      <p className="mb-2">
        GET /api/health (proxied to the Axum API via Next.js rewrite):
      </p>
      <pre className="bg-zinc-100 dark:bg-zinc-900 p-4 rounded">
        {health}
      </pre>
    </main>
  );
}
