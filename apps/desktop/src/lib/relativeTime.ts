/** Visible relative time. The full clock time is separate, for hover and screen readers. */
export function formatRelativeTime(thenMs: number, nowMs = Date.now()): string {
  const diff = nowMs - thenMs;
  if (!Number.isFinite(diff) || diff < 60_000) return "just now";
  if (diff < 3_600_000) {
    const minutes = Math.floor(diff / 60_000);
    return minutes === 1 ? "1 minute ago" : `${minutes} minutes ago`;
  }
  if (diff < 86_400_000) {
    const hours = Math.floor(diff / 3_600_000);
    return hours === 1 ? "1 hour ago" : `${hours} hours ago`;
  }
  const days = Math.floor(diff / 86_400_000);
  return days === 1 ? "1 day ago" : `${days} days ago`;
}
