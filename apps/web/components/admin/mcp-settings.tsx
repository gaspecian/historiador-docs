"use client";

import { useState } from "react";
import { apiFetch } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/ui/copy-button";
import type { RegenerateTokenResponse, WorkspaceResponse } from "@/lib/types";

interface Props {
  workspace: WorkspaceResponse;
}

export function McpSettings({ workspace }: Props) {
  const [token, setToken] = useState<string | null>(null);
  const [showToken, setShowToken] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleRegenerate = async () => {
    if (!confirm("Regenerate the MCP bearer token? The old token will stop working immediately.")) return;
    setLoading(true);
    try {
      const data = await apiFetch<RegenerateTokenResponse>("/admin/workspace/regenerate-token", {
        method: "POST",
      });
      setToken(data.bearer_token);
      setShowToken(true);
    } catch {
      // Alpha error handling
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* MCP Endpoint URL */}
      <div className="space-y-1">
        <label className="block text-sm font-medium text-zinc-700 dark:text-zinc-300">
          MCP Endpoint URL
        </label>
        <div className="flex items-center gap-2">
          <code className="flex-1 text-sm bg-zinc-50 dark:bg-zinc-800 p-2 rounded border border-zinc-200 dark:border-zinc-700">
            {workspace.mcp_endpoint_url}
          </code>
          <CopyButton text={workspace.mcp_endpoint_url} />
        </div>
      </div>

      {/* Bearer Token */}
      <div className="space-y-1">
        <label className="block text-sm font-medium text-zinc-700 dark:text-zinc-300">
          Bearer Token
        </label>
        {token ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <code className="flex-1 text-sm bg-zinc-50 dark:bg-zinc-800 p-2 rounded border border-zinc-200 dark:border-zinc-700 break-all">
                {showToken ? token : "\u2022".repeat(32)}
              </code>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowToken(!showToken)}
              >
                {showToken ? "Hide" : "Show"}
              </Button>
              <CopyButton text={token} />
            </div>
            <p className="text-xs text-yellow-600 dark:text-yellow-400">
              Save this token now. It will not be shown again.
            </p>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <span className="text-sm text-zinc-500">
              {workspace.has_mcp_token ? "Token is set" : "No token configured"}
            </span>
          </div>
        )}
        <Button
          variant="secondary"
          size="sm"
          onClick={handleRegenerate}
          disabled={loading}
        >
          {loading ? "Regenerating..." : "Regenerate Token"}
        </Button>
      </div>
    </div>
  );
}
