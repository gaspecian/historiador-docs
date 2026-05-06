"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import * as exportService from "@/lib/services/export";

export function ExportSection() {
 const [loading, setLoading] = useState(false);
 const [error, setError] = useState<string | null>(null);

 const download = async () => {
 setLoading(true);
 setError(null);
 try {
 await exportService.workspaceZip();
 } catch (e) {
 setError(e instanceof Error ? e.message : String(e));
 } finally {
 setLoading(false);
 }
 };

 return (
 <div className="space-y-2 text-sm">
 <p className="text-text-secondary">
 Baixe todas as páginas publicadas em todos os idiomas como um zip de
 arquivos markdown, organizados pela hierarquia de coleções. Cada
 arquivo carrega front-matter YAML para compatibilidade com ferramentas
 docs-as-code.
 </p>
 <Button variant="secondary" size="sm" onClick={download} disabled={loading}>
 {loading ? (
 <>
 <Spinner className="mr-2" /> Preparando zip…
 </>
 ) : (
 "Baixar workspace como markdown"
 )}
 </Button>
 {error && <p className="text-red-600">Falha na exportação: {error}</p>}
 </div>
 );
}
