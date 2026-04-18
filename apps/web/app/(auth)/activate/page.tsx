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
 setError("Password must be at least 12 characters");
 return;
 }
 if (password !== confirm) {
 setError("Passwords do not match");
 return;
 }
 if (!token) {
 setError("Missing activation token");
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
 setError(err instanceof Error ? err.message : "Activation failed");
 } finally {
 setLoading(false);
 }
 };

 if (success) {
 return (
 <main className="flex min-h-screen items-center justify-center p-4">
 <div className="text-center space-y-2">
 <h1 className="text-xl font-bold">Account activated</h1>
 <p className="text-sm text-text-tertiary">Redirecting to login...</p>
 </div>
 </main>
 );
 }

 return (
 <main className="flex min-h-screen items-center justify-center p-4">
 <div className="w-full max-w-sm space-y-6">
 <div className="text-center">
 <h1 className="text-2xl font-bold">Activate your account</h1>
 <p className="mt-1 text-sm text-text-tertiary">
 Set a password to complete your registration
 </p>
 </div>

 <form onSubmit={handleSubmit} className="space-y-4">
 <Input
 label="Password"
 type="password"
 value={password}
 onChange={(e) => setPassword(e.target.value)}
 placeholder="Min. 12 characters"
 required
 autoComplete="new-password"
 />
 <Input
 label="Confirm password"
 type="password"
 value={confirm}
 onChange={(e) => setConfirm(e.target.value)}
 required
 autoComplete="new-password"
 />

 {error && <p className="text-sm text-red-600">{error}</p>}

 <Button type="submit" disabled={loading} className="w-full">
 {loading ? "Activating..." : "Activate account"}
 </Button>
 </form>
 </div>
 </main>
 );
}
