"use client";

import { Suspense, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";

export default function ActivatePage() {
 return (
 <Suspense
 fallback={
 <main className="flex min-h-screen items-center justify-center">
 <Spinner />
 </main>
 }
 >
 <ActivateForm />
 </Suspense>
 );
}

function ActivateForm() {
 const router = useRouter();
 const searchParams = useSearchParams();
 const token = searchParams.get("token") || "";

 const [password, setPassword] = useState("");
 const [confirm, setConfirm] = useState("");
 const [error, setError] = useState("");
 const [loading, setLoading] = useState(false);
 const [success, setSuccess] = useState(false);

 const handleSubmit = async (e: React.FormEvent) => {
 e.preventDefault();
 setError("");

 if (password.length < 12) {
 setError("A senha precisa ter no mínimo 12 caracteres");
 return;
 }
 if (password !== confirm) {
 setError("As senhas não coincidem");
 return;
 }
 if (!token) {
 setError("Token de ativação ausente");
 return;
 }

 setLoading(true);
 try {
 const res = await fetch("/api/auth/activate", {
 method: "POST",
 headers: { "Content-Type": "application/json" },
 body: JSON.stringify({ invite_token: token, password }),
 });

 if (!res.ok) {
 const body = await res.text();
 let message: string;
 try {
 message = JSON.parse(body).message || body;
 } catch {
 message = body;
 }
 throw new Error(message);
 }

 setSuccess(true);
 setTimeout(() => router.push("/login"), 2000);
 } catch (err) {
 setError(err instanceof Error ? err.message : "Falha ao ativar a conta");
 } finally {
 setLoading(false);
 }
 };

 if (success) {
 return (
 <main className="flex min-h-screen items-center justify-center p-4">
 <div className="text-center space-y-2">
 <h1 className="text-xl font-bold">Conta ativada</h1>
 <p className="text-sm text-text-tertiary">Redirecionando para o login…</p>
 </div>
 </main>
 );
 }

 return (
 <main className="flex min-h-screen items-center justify-center p-4">
 <div className="w-full max-w-sm space-y-6">
 <div className="text-center">
 <h1 className="text-2xl font-bold">Ative sua conta</h1>
 <p className="mt-1 text-sm text-text-tertiary">
 Defina uma senha para concluir seu cadastro
 </p>
 </div>

 <form onSubmit={handleSubmit} className="space-y-4">
 <Input
 label="Senha"
 type="password"
 value={password}
 onChange={(e) => setPassword(e.target.value)}
 placeholder="Mín. 12 caracteres"
 required
 autoComplete="new-password"
 />
 <Input
 label="Confirmar senha"
 type="password"
 value={confirm}
 onChange={(e) => setConfirm(e.target.value)}
 required
 autoComplete="new-password"
 />

 {error && <p className="text-sm text-red-600">{error}</p>}

 <Button type="submit" disabled={loading} className="w-full">
 {loading ? "Ativando…" : "Ativar conta"}
 </Button>
 </form>
 </div>
 </main>
 );
}
