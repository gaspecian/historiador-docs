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
 const [deactivating, setDeactivating] = useState<string | null>(null);

 const handleDeactivate = async (userId: string) => {
 if (!confirm("Deactivate this user?")) return;
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
 return <p className="text-sm text-text-tertiary">No users found.</p>;
 }

 return (
 <div className="border border-surface-border rounded overflow-hidden">
 <table className="w-full text-sm">
 <thead className="bg-surface-subtle">
 <tr>
 <th className="text-left px-4 py-2 font-medium text-text-secondary">Email</th>
 <th className="text-left px-4 py-2 font-medium text-text-secondary">Role</th>
 <th className="text-left px-4 py-2 font-medium text-text-secondary">Status</th>
 <th className="px-4 py-2" />
 </tr>
 </thead>
 <tbody className="divide-y divide-zinc-200">
 {users.map((user) => (
 <tr key={user.id} className="hover:bg-surface-subtle">
 <td className="px-4 py-2">{user.email}</td>
 <td className="px-4 py-2">
 <Badge variant={user.role === "admin" ? "warning" : user.role === "author" ? "success" : "neutral"}>
 {user.role}
 </Badge>
 </td>
 <td className="px-4 py-2">
 {user.pending ? (
 <Badge variant="warning">Pending</Badge>
 ) : user.active ? (
 <Badge variant="success">Active</Badge>
 ) : (
 <Badge variant="danger">Deactivated</Badge>
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
 {deactivating === user.id ? "..." : "Deactivate"}
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
