import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { useFocusStore } from "@commandui/state";

export type InputComposerHandle = {
  focus: () => void;
};

type Props = {
  mode: "command" | "ask";
  onModeChange: (mode: "command" | "ask") => void;
  onSubmit: (value: string) => void;
  busy?: boolean;
  isRunning?: boolean;
  onInterrupt?: () => void;
  disabled?: boolean;
};

export const InputComposer = forwardRef<InputComposerHandle, Props>(
  function InputComposer(
    {
      mode,
      onModeChange,
      onSubmit,
      busy = false,
      isRunning = false,
      onInterrupt,
      disabled = false,
    },
    ref,
  ) {
    const [value, setValue] = useState("");
    const textareaRef = useRef<HTMLTextAreaElement | null>(null);
    const setFocusZone = useFocusStore((s) => s.setFocusZone);

    useImperativeHandle(ref, () => ({
      focus() {
        textareaRef.current?.focus();
      },
    }));

    useEffect(() => {
      textareaRef.current?.focus();
    }, [mode]);

    // Auto-resize textarea to content
    useEffect(() => {
      const el = textareaRef.current;
      if (!el) return;
      el.style.height = "auto";
      el.style.height = `${el.scrollHeight}px`;
    }, [value]);

    const cantSubmit = busy || isRunning || disabled;

    function handleSubmit() {
      const trimmed = value.trim();
      if (!trimmed || cantSubmit) return;
      onSubmit(trimmed);
      setValue("");
      textareaRef.current?.focus();
    }

    function handleKeyDown(e: React.KeyboardEvent) {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
      // Shift+Enter: default textarea behavior (newline)
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

        <textarea
          ref={textareaRef}
          className="composer-input"
          rows={1}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => setFocusZone("composer")}
          placeholder={
            isRunning
              ? "Command running…"
              : mode === "command"
                ? "Submit a command explicitly…"
                : "Describe what you want to do…"
          }
          disabled={isRunning || disabled}
        />

        {isRunning ? (
          <button
            type="button"
            className="btn-stop"
            onClick={() => onInterrupt?.()}
          >
            Stop
          </button>
        ) : (
          <button type="button" onClick={handleSubmit} disabled={cantSubmit}>
            {busy ? "Working…" : "Run"}
          </button>
        )}
      </div>
    );
  },
);
