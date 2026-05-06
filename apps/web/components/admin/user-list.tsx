"use client";

import { useState } from "react";
import * as adminService from "@/lib/services/admin";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { UserResponse } from "@historiador/types";

interface Props {
 users: UserResponse[];
 onRefresh: () => void;
}

export function UserList({ users, onRefresh }: Props) {
 const ROLE_LABELS: Record<string, string> = {
  admin: "Administrador",
  author: "Autor",
  viewer: "Leitor",
 };
 const [deactivating, setDeactivating] = useState<string | null>(null);

 const handleDeactivate = async (userId: string) => {
 if (!confirm("Desativar este usuário?")) return;
 setDeactivating(userId);
 try {
 await adminService.deactivateUser(userId);
 onRefresh();
 } catch {
 // Alpha error handling
 } finally {
 setDeactivating(null);
 }
 };

 if (users.length === 0) {
 return <p className="text-sm text-text-tertiary">Nenhum usuário encontrado.</p>;
 }

 return (
 <div className="border border-surface-border rounded overflow-hidden">
 <table className="w-full text-sm">
 <thead className="bg-surface-subtle">
 <tr>
 <th className="text-left px-4 py-2 font-medium text-text-secondary">E-mail</th>
 <th className="text-left px-4 py-2 font-medium text-text-secondary">Perfil</th>
 <th className="text-left px-4 py-2 font-medium text-text-secondary">Situação</th>
 <th className="px-4 py-2" />
 </tr>
 </thead>
 <tbody className="divide-y divide-zinc-200">
 {users.map((user) => (
 <tr key={user.id} className="hover:bg-surface-subtle">
 <td className="px-4 py-2">{user.email}</td>
 <td className="px-4 py-2">
 <Badge variant={user.role === "admin" ? "warning" : user.role === "author" ? "success" : "neutral"}>
 {ROLE_LABELS[user.role] ?? user.role}
 </Badge>
 </td>
 <td className="px-4 py-2">
 {user.pending ? (
 <Badge variant="warning">Pendente</Badge>
 ) : user.active ? (
 <Badge variant="success">Ativo</Badge>
 ) : (
 <Badge variant="danger">Desativado</Badge>
 )}
 </td>
 <td className="px-4 py-2 text-right">
 {user.active && !user.pending && (
 <Button
 variant="ghost"
 size="sm"
 onClick={() => handleDeactivate(user.id)}
 disabled={deactivating === user.id}
 >
 {deactivating === user.id ? "..." : "Desativar"}
 </Button>
 )}
 </td>
 </tr>
 ))}
 </tbody>
 </table>
 </div>
 );
}
