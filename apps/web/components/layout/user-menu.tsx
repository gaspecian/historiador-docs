"use client";

import { useAuth } from "@/lib/auth-context";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

export function UserMenu() {
 const { user, logout } = useAuth();

 if (!user) return null;

 const roleBadgeVariant = user.role === "admin" ? "warning" : user.role === "author" ? "success" : "neutral";

 return (
 <div className="flex items-center gap-3">
 <div className="flex items-center gap-2 text-sm">
 <span className="text-text-secondary">{user.email || "User"}</span>
 <Badge variant={roleBadgeVariant}>{user.role}</Badge>
 </div>
 <Button variant="ghost" size="sm" onClick={logout}>
 Sign out
 </Button>
 </div>
 );
}
