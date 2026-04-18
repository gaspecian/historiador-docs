"use client";

import { useCallback, useEffect, useState } from "react";
import { apiFetch } from "@/lib/api";
import { Spinner } from "@/components/ui/spinner";
import { Button } from "@/components/ui/button";
import type { McpAnalyticsResponse } from "@historiador/types";

export function McpAnalytics() {
 const [data, setData] = useState<McpAnalyticsResponse | null>(null);
 const [loading, setLoading] = useState(true);
 const [days, setDays] = useState(7);
 const [error, setError] = useState<string | null>(null);

 const fetchAnalytics = useCallback(async () => {
 setLoading(true);
 setError(null);
 try {
 const result = await apiFetch<McpAnalyticsResponse>(
 `/admin/analytics/mcp-queries?days=${days}`,
 );
 setData(result);
 } catch (e) {
 setError(e instanceof Error ? e.message : "Failed to load analytics");
 } finally {
 setLoading(false);
 }
 }, [days]);

 useEffect(() => {
 fetchAnalytics();
 }, [fetchAnalytics]);

 if (loading) {
 return (
 <div className="flex justify-center py-4">
 <Spinner />
 </div>
 );
 }

 if (error) {
 return (
 <p className="text-sm text-text-tertiary py-4">{error}</p>
 );
 }

 if (!data) return null;

 return (
 <div className="space-y-6">
 {/* Period toggle */}
 <div className="flex gap-2">
 <Button
 variant={days === 7 ? "primary" : "secondary"}
 size="sm"
 onClick={() => setDays(7)}
 >
 7 days
 </Button>
 <Button
 variant={days === 30 ? "primary" : "secondary"}
 size="sm"
 onClick={() => setDays(30)}
 >
 30 days
 </Button>
 </div>

 {/* Summary card */}
 <div className="rounded border border-surface-border p-4">
 <p className="text-2xl font-bold">{data.total_queries}</p>
 <p className="text-sm text-text-tertiary">
 Total queries ({data.period_days}d)
 </p>
 </div>

 {/* Queries by day */}
 {data.queries_by_day.length > 0 && (
 <div>
 <h3 className="text-sm font-medium mb-2">Queries by day</h3>
 <table className="w-full text-sm">
 <thead>
 <tr className="border-b border-surface-border">
 <th className="text-left py-1 font-medium">Date</th>
 <th className="text-right py-1 font-medium">Count</th>
 <th className="text-left py-1 pl-3 font-medium w-1/2">
 Volume
 </th>
 </tr>
 </thead>
 <tbody>
 {data.queries_by_day.map((d) => {
 const maxCount = Math.max(
 ...data.queries_by_day.map((x) => x.count),
 1,
 );
 const pct = (d.count / maxCount) * 100;
 return (
 <tr
 key={d.date}
 className="border-b border-surface-border"
 >
 <td className="py-1.5 font-mono text-xs">
 {d.date.split("T")[0]}
 </td>
 <td className="py-1.5 text-right">{d.count}</td>
 <td className="py-1.5 pl-3">
 <div
 className="h-3 rounded bg-primary-500"
 style={{ width: `${pct}%` }}
 />
 </td>
 </tr>
 );
 })}
 </tbody>
 </table>
 </div>
 )}

 {/* Top query topics */}
 {data.top_queries.length > 0 && (
 <div>
 <h3 className="text-sm font-medium mb-2">Top query topics</h3>
 <table className="w-full text-sm">
 <thead>
 <tr className="border-b border-surface-border">
 <th className="text-left py-1 font-medium">Query</th>
 <th className="text-right py-1 font-medium">Count</th>
 </tr>
 </thead>
 <tbody>
 {data.top_queries.map((q, i) => (
 <tr
 key={i}
 className="border-b border-surface-border"
 >
 <td className="py-1.5 truncate max-w-xs">{q.query_text}</td>
 <td className="py-1.5 text-right">{q.count}</td>
 </tr>
 ))}
 </tbody>
 </table>
 </div>
 )}

 {/* Zero-result queries */}
 {data.zero_result_queries.count > 0 && (
 <div>
 <h3 className="text-sm font-medium mb-2">
 Zero-result queries ({data.zero_result_queries.count})
 </h3>
 <p className="text-xs text-text-tertiary mb-2">
 Queries that returned no results — potential documentation gaps.
 </p>
 <table className="w-full text-sm">
 <thead>
 <tr className="border-b border-surface-border">
 <th className="text-left py-1 font-medium">Query</th>
 <th className="text-right py-1 font-medium">Count</th>
 <th className="text-right py-1 font-medium">Last seen</th>
 </tr>
 </thead>
 <tbody>
 {data.zero_result_queries.queries.map((q, i) => (
 <tr
 key={i}
 className="border-b border-surface-border"
 >
 <td className="py-1.5 truncate max-w-xs">{q.query_text}</td>
 <td className="py-1.5 text-right">{q.count}</td>
 <td className="py-1.5 text-right text-xs text-text-tertiary">
 {q.last_seen ? q.last_seen.split("T")[0] : "—"}
 </td>
 </tr>
 ))}
 </tbody>
 </table>
 </div>
 )}

 {data.total_queries === 0 && (
 <p className="text-sm text-text-tertiary text-center py-4">
 No MCP queries recorded yet. Queries will appear here once the MCP
 endpoint receives traffic.
 </p>
 )}
 </div>
 );
}
