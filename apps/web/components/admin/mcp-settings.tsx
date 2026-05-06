"use client";

import { useState } from "react";
import * as adminService from "@/lib/services/admin";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/ui/copy-button";
import type { WorkspaceResponse } from "@historiador/types";

interface Props {
 workspace: WorkspaceResponse;
}

export function McpSettings({ workspace }: Props) {
 const [token, setToken] = useState<string | null>(null);
 const [showToken, setShowToken] = useState(false);
 const [loading, setLoading] = useState(false);

 const handleRegenerate = async () => {
 if (!confirm("Regenerar o token bearer do MCP? O token antigo deixará de funcionar imediatamente.")) return;
 setLoading(true);
 try {
 const data = await adminService.regenerateToken();
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
 <label className="block text-sm font-medium text-text-secondary">
 URL do endpoint MCP
 </label>
 <div className="flex items-center gap-2">
 <code className="flex-1 text-sm bg-surface-subtle p-2 rounded border border-surface-border">
 {workspace.mcp_endpoint_url}
 </code>
 <CopyButton text={workspace.mcp_endpoint_url} />
 </div>
 </div>

 {/* Bearer Token */}
 <div className="space-y-1">
 <label className="block text-sm font-medium text-text-secondary">
 Token Bearer
 </label>
 {token ? (
 <div className="space-y-2">
 <div className="flex items-center gap-2">
 <code className="flex-1 text-sm bg-surface-subtle p-2 rounded border border-surface-border break-all">
 {showToken ? token : "\u2022".repeat(32)}
 </code>
 <Button
 variant="ghost"
 size="sm"
 onClick={() => setShowToken(!showToken)}
 >
 {showToken ? "Ocultar" : "Mostrar"}
 </Button>
 <CopyButton text={token} />
 </div>
 <p className="text-xs text-amber-600">
 Salve este token agora. Ele não será exibido novamente.
 </p>
 </div>
 ) : (
 <div className="flex items-center gap-2">
 <span className="text-sm text-text-tertiary">
 {workspace.has_mcp_token ? "Token configurado" : "Nenhum token configurado"}
 </span>
 </div>
 )}
 <Button
 variant="secondary"
 size="sm"
 onClick={handleRegenerate}
 disabled={loading}
 >
 {loading ? "Regenerando…" : "Regenerar token"}
 </Button>
 </div>
 </div>
 );
}
