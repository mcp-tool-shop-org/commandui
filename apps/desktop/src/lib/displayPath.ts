/**
 * How a folder is shown in the UI: the user's home folder as `~`, and a long
 * path shortened in the middle so the folder you are in stays readable.
 *
 * History uses the same shortening. The shell's own output is unchanged.
 * Showing `~` keeps the account name off the screen, which matters in
 * screenshots and screen shares.
 */

const WINDOWS_HOME = /^([A-Za-z]:[\\/](?:Users|Documents and Settings)[\\/][^\\/]+)(?=[\\/]|$)/i;
const UNIX_HOME = /^(\/(?:home|Users)\/[^/]+|\/root)(?=\/|$)/;

/** The path with a leading home folder replaced by `~`. */
export function tildeHome(path: string): string {
  const m = WINDOWS_HOME.exec(path) ?? UNIX_HOME.exec(path);
  if (!m) return path;
  return `~${path.slice(m[1].length)}`;
}

/**
 * `tildeHome`, then, when still longer than `max` characters, the start and
 * the last two folders with `…` between: `~\…\acme-api\src`.
 */
export function displayPath(path: string | null | undefined, max = 48): string {
  if (!path) return "";
  const short = tildeHome(path);
  if (short.length <= max) return short;
  const sep = short.includes("\\") ? "\\" : "/";
  const parts = short.split(sep).filter((p, i) => p !== "" || i === 0);
  if (parts.length <= 3) return short;
  const head = parts[0] === "" ? "" : parts[0];
  const tail = parts.slice(-2).join(sep);
  return `${head}${sep}…${sep}${tail}`;
}
