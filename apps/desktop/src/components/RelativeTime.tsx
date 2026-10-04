import { formatRelativeTime } from "../lib/relativeTime";

/** Relative text on screen, the full date on hover and for a screen reader. */
export function RelativeTime({ value }: { value: string | number }) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return <span>{String(value)}</span>;
  }
  const full = date.toLocaleString();
  return (
    <time dateTime={date.toISOString()} title={full}>
      {formatRelativeTime(date.getTime())}
      <span className="visually-hidden">, {full}</span>
    </time>
  );
}
