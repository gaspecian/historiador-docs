"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { apiDownload } from "@/lib/api";

export function ExportSection() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const download = async () => {
    setLoading(true);
    setError(null);
    try {
      await apiDownload("/export");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-2 text-sm">
      <p className="text-zinc-600 dark:text-zinc-400">
        Download every published page in every language as a zip of markdown
        files, organized by collection hierarchy. Each file carries YAML
        front-matter for round-trip compatibility with docs-as-code tooling.
      </p>
      <Button variant="secondary" size="sm" onClick={download} disabled={loading}>
        {loading ? (
          <>
            <Spinner className="mr-2" /> Preparing zip…
          </>
        ) : (
          "Download workspace as markdown"
        )}
      </Button>
      {error && <p className="text-red-600">Export failed: {error}</p>}
    </div>
  );
}
