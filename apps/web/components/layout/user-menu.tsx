"use client";

import { useAuth } from "@/lib/auth-context";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

export function UserMenu() {
 const { user, logout } = useAuth();

 if (!user) return null;

 const roleBadgeVariant = user.role === "admin" ? "warning" : user.role === "author" ? "success" : "neutral";

 const ROLE_LABELS: Record<string, string> = {
  admin: "Administrador",
  author: "Autor",
  viewer: "Leitor",
 };

 return (
 <div className="flex items-center gap-3">
 <div className="flex items-center gap-2 text-sm">
 <span className="text-text-secondary">{user.email || "Usuário"}</span>
 <Badge variant={roleBadgeVariant}>{ROLE_LABELS[user.role] ?? user.role}</Badge>
 </div>
 <Button variant="ghost" size="sm" onClick={logout}>
 Sair
 </Button>
 </div>
 );
}
