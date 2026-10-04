/** Same hexes as --term-bg and --term-fg in globals.css. */
export const TERMINAL_DARK = { background: "#171c22", foreground: "#e8ebf0" };
export const TERMINAL_LIGHT = { background: "#ffffff", foreground: "#111827" };

export function terminalTheme(light: boolean): { background: string; foreground: string } {
  return light ? TERMINAL_LIGHT : TERMINAL_DARK;
}
