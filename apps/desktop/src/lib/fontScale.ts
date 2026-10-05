/**
 * The stored font size was never drawn, so a saved "md" must stay at the
 * size the screen already has. "lg" is the old large step. The slider
 * stores a percent from 100 to 200 in the same field.
 */
export function fontScale(fontSize: string | null | undefined): number {
  if (fontSize == null) return 1;
  const value = fontSize.trim().toLowerCase();
  if (value === "lg") return 2;
  if (value === "sm" || value === "md" || value === "") return 1;
  const percent = Number(value);
  if (!Number.isFinite(percent)) return 1;
  return Math.min(2, Math.max(1, percent / 100));
}

export function fontSizePercent(fontSize: string | null | undefined): number {
  return Math.round(fontScale(fontSize) * 100);
}
