"use client";

/**
 * Save-document dialog (Sprint 11).
 *
 * Opens when the user hits "Salvar" in the editor top bar. Collects
 * the title + target collection and calls `POST /pages` with the
 * current draft markdown. The collection dropdown renders the
 * workspace tree flat with indentation prefixes so a nested
 * location is one click away.
 */

import { useEffect, useMemo, useState } from "react";
import { Loader2, Save } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import * as pagesService from "@/lib/services/pages";
import { useCollectionsQuery } from "@/lib/queries/use-collections";
import type { PageResponse, TreeNode } from "@historiador/types";

export interface SaveDialogProps {
  open: boolean;
  /** Draft markdown to persist. */
  markdown: string;
  /** BCP 47 language tag; defaults to pt-BR. */
  language?: string;
  /** When set, Salvar calls PATCH /pages/:id instead of POST /pages. */
  pageId?: string | null;
  /** Initial title when updating — typically the existing version's title. */
  initialTitle?: string;
  onClose: () => void;
  onSaved?: (page: PageResponse) => void;
}

export function SaveDialog({
  open,
  markdown,
  language = "pt-BR",
  pageId,
  initialTitle,
  onClose,
  onSaved,
}: SaveDialogProps) {
  const isUpdating = Boolean(pageId);
  const [title, setTitle] = useState(initialTitle ?? "");
  const [collectionId, setCollectionId] = useState<string>("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const collections = useCollectionsQuery();

  const options = useMemo(() => {
    const flat: Array<{ value: string; label: string }> = [
      { value: "", label: "— Raiz (sem coleção)" },
    ];
    const walk = (nodes: TreeNode[], depth: number) => {
      for (const node of nodes) {
        const indent = "  ".repeat(depth);
        flat.push({ value: node.id, label: `${indent}${node.name}` });
        if (node.children && node.children.length > 0) {
          walk(node.children, depth + 1);
        }
      }
    };
    walk(collections.tree, 0);
    return flat;
  }, [collections.tree]);

  useEffect(() => {
    if (!open) {
      setTitle(initialTitle ?? "");
      setCollectionId("");
      setError(null);
      setSaving(false);
    }
  }, [open, initialTitle]);

  const submit = async () => {
    const trimmed = title.trim();
    if (!trimmed) {
      setError("Dê um título ao documento antes de salvar.");
      return;
    }
    if (!markdown.trim()) {
      setError("O rascunho está vazio — escreva algo antes de salvar.");
      return;
    }
    setError(null);
    setSaving(true);
    try {
      const page = isUpdating && pageId
        ? await pagesService.update(pageId, {
            title: trimmed,
            content_markdown: markdown,
            language,
          })
        : await pagesService.create({
            title: trimmed,
            content_markdown: markdown,
            language,
            collection_id: collectionId.length > 0 ? collectionId : null,
          });
      onSaved?.(page);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Não foi possível salvar.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onClose={onClose}>
      <div className="flex flex-col gap-4">
        <div>
          <h2 className="t-h3 mb-1">
            {isUpdating ? "Atualizar documento" : "Salvar documento"}
          </h2>
          <p className="t-body-sm text-text-secondary">
            {isUpdating
              ? "Você pode renomear o título; o conteúdo substitui a versão atual."
              : "Escolha um título e onde o documento vai ficar."}
          </p>
        </div>

        <Input
          label="Título"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="ex.: Fettuccine a Carbonara"
          disabled={saving}
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void submit();
            }
          }}
        />

        {!isUpdating && (
          <Select
            label="Local (coleção)"
            value={collectionId}
            onChange={(e) => setCollectionId(e.target.value)}
            options={options}
            disabled={saving || collections.isLoading}
          />
        )}

        {error && (
          <p className="t-body-sm text-red-600 whitespace-pre-wrap">{error}</p>
        )}

        <div className="flex justify-end gap-2 pt-2">
          <Button variant="secondary" onClick={onClose} disabled={saving}>
            Cancelar
          </Button>
          <Button onClick={() => void submit()} disabled={saving || !title.trim()}>
            <span className="inline-flex items-center gap-1.5">
              {saving ? (
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden />
              ) : (
                <Save className="w-4 h-4" aria-hidden />
              )}
              {saving ? "Salvando…" : "Salvar"}
            </span>
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
