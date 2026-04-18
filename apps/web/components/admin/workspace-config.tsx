"use client";

import { Badge } from "@/components/ui/badge";
import type { WorkspaceResponse } from "@historiador/types";

interface Props {
 workspace: WorkspaceResponse;
}

export function WorkspaceConfig({ workspace }: Props) {
 return (
 <div className="space-y-3">
 <div className="grid grid-cols-2 gap-4 text-sm">
 <div>
 <span className="font-medium text-text-secondary">Workspace name</span>
 <p>{workspace.name}</p>
 </div>
 <div>
 <span className="font-medium text-text-secondary">LLM Provider</span>
 <p className="capitalize">{workspace.llm_provider}</p>
 </div>
 <div>
 <span className="font-medium text-text-secondary">Generation model</span>
 <p className="font-mono text-xs">{workspace.generation_model}</p>
 </div>
 <div>
 <span className="font-medium text-text-secondary">Embedding model</span>
 <p className="font-mono text-xs">{workspace.embedding_model}</p>
 </div>
 <div>
 <span className="font-medium text-text-secondary">Primary language</span>
 <p>
 <Badge variant="success">{workspace.primary_language}</Badge>
 </p>
 </div>
 <div>
 <span className="font-medium text-text-secondary">Languages</span>
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
