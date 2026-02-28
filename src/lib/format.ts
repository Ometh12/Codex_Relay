export function formatTimeMs(ms?: number | null): string {
  if (!ms) return "-";
  try {
    return new Date(ms).toLocaleString("zh-CN");
  } catch {
    return String(ms);
  }
}

export function formatRfc3339(ts?: string | null): string {
  if (!ts) return "-";
  const ms = Date.parse(ts);
  if (!Number.isFinite(ms)) return ts;
  return new Date(ms).toLocaleString("zh-CN");
}

export function formatBytes(n?: number | null): string {
  if (!n || n <= 0) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  const digits = i === 0 ? 0 : i === 1 ? 1 : 2;
  return `${v.toFixed(digits)} ${units[i]}`;
}

export function shortSha(sha?: string | null): string {
  if (!sha) return "-";
  if (sha.length <= 12) return sha;
  return `${sha.slice(0, 12)}…`;
}
