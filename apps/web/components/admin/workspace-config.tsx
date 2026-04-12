"use client";

import { Badge } from "@/components/ui/badge";
import type { WorkspaceResponse } from "@/lib/types";

interface Props {
  workspace: WorkspaceResponse;
}

export function WorkspaceConfig({ workspace }: Props) {
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-4 text-sm">
        <div>
          <span className="font-medium text-zinc-600 dark:text-zinc-400">Workspace name</span>
          <p>{workspace.name}</p>
        </div>
        <div>
          <span className="font-medium text-zinc-600 dark:text-zinc-400">LLM Provider</span>
          <p className="capitalize">{workspace.llm_provider}</p>
        </div>
        <div>
          <span className="font-medium text-zinc-600 dark:text-zinc-400">Primary language</span>
          <p>
            <Badge variant="success">{workspace.primary_language}</Badge>
          </p>
        </div>
        <div>
          <span className="font-medium text-zinc-600 dark:text-zinc-400">Languages</span>
          <div className="flex gap-1 mt-1 flex-wrap">
            {workspace.languages.map((lang) => (
              <Badge
                key={lang}
                variant={lang === workspace.primary_language ? "success" : "neutral"}
              >
                {lang}
              </Badge>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
