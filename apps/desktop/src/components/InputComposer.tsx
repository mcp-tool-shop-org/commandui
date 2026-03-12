import { useEffect, useRef, useState } from "react";

type Props = {
  mode: "command" | "ask";
  onModeChange: (mode: "command" | "ask") => void;
  onSubmit: (value: string) => void;
  busy?: boolean;
};

export function InputComposer({
  mode,
  onModeChange,
  onSubmit,
  busy = false,
}: Props) {
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, [mode]);

  function handleSubmit() {
    const trimmed = value.trim();
    if (!trimmed || busy) return;
    onSubmit(trimmed);
    setValue("");
    inputRef.current?.focus();
  }

  return (
    <div className="composer">
      <div className="mode-toggle">
        <button
          className={mode === "command" ? "active" : ""}
          onClick={() => onModeChange("command")}
          type="button"
        >
          Command
        </button>

        <button
          className={mode === "ask" ? "active" : ""}
          onClick={() => onModeChange("ask")}
          type="button"
        >
          Ask
        </button>
      </div>

      <input
        ref={inputRef}
        className="composer-input"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") handleSubmit();
        }}
        placeholder={
          mode === "command"
            ? "Submit a command explicitly…"
            : "Describe what you want to do…"
        }
      />

      <button type="button" onClick={handleSubmit} disabled={busy}>
        {busy ? "Working…" : "Run"}
      </button>
    </div>
  );
}
