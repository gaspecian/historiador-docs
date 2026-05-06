"use client";

import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface Props {
 open: boolean;
 missingLanguages: string[];
 onConfirm: () => void;
 onCancel: () => void;
}

export function PublishConfirmModal({ open, missingLanguages, onConfirm, onCancel }: Props) {
 return (
 <Dialog open={open} onClose={onCancel}>
 <div className="space-y-4">
 <h3 className="text-lg font-semibold">Cobertura de idiomas incompleta</h3>
 <p className="text-sm text-text-secondary">
 Os seguintes idiomas estão sem versão:
 </p>
 <ul className="list-disc list-inside text-sm space-y-1">
 {missingLanguages.map((lang) => (
 <li key={lang} className="text-amber-700 font-medium">
 {lang}
 </li>
 ))}
 </ul>
 <div className="flex justify-end gap-3 pt-2">
 <Button variant="secondary" size="sm" onClick={onCancel}>
 Voltar
 </Button>
 <Button variant="primary" size="sm" onClick={onConfirm}>
 Publicar mesmo assim
 </Button>
 </div>
 </div>
 </Dialog>
 );
}
