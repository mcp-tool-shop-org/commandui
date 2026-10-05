import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { useFocusStore } from "@commandui/state";

export type InputComposerHandle = {
  focus: () => void;
  /** Replace the draft. A result action uses this to move text into Ask. */
  setValue: (value: string) => void;
};

type Props = {
  mode: "command" | "ask";
  onModeChange: (mode: "command" | "ask") => void;
  /**
   * Return false (or a promise of false) when the submit was rejected, so the
   * composer keeps what the user typed instead of discarding it.
   */
  onSubmit: (value: string) => void | boolean | Promise<boolean | void>;
  busy?: boolean;
  isRunning?: boolean;
  onInterrupt?: () => void;
  disabled?: boolean;
  /** Why the composer is disabled, shown as its placeholder. */
  disabledReason?: string;
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
      disabledReason,
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
      setValue(next: string) {
        setValue(next);
      },
    }));

    useEffect(() => {
      textareaRef.current?.focus();
    }, [mode]);

    // Auto-resize textarea to content
    useEffect(() => {
      const el = textareaRef.current;
      if (!el) return;
      const lines = value.split("\n").length;
      el.rows = Math.min(12, Math.max(1, lines));
    }, [value]);

    const cantSubmit = busy || isRunning || disabled;

    function handleSubmit() {
      const trimmed = value.trim();
      if (!trimmed || cantSubmit) return;
      const result = onSubmit(trimmed);
      if (result === false) {
        // Rejected synchronously: keep the text.
        textareaRef.current?.focus();
        return;
      }
      setValue("");
      if (result && typeof (result as Promise<unknown>).then === "function") {
        // Rejected after an await: put the text back unless the user already typed something new.
        void (result as Promise<boolean | void>).then(
          (ok) => {
            if (ok === false) setValue((current) => (current === "" ? trimmed : current));
          },
          () => setValue((current) => (current === "" ? trimmed : current)),
        );
      }
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
        <div className="mode-toggle" role="group" aria-label="How to send this">
          <button
            className={mode === "command" ? "active" : ""}
            onClick={() => onModeChange("command")}
            type="button"
            aria-pressed={mode === "command"}
          >
            Command
          </button>

          <button
            className={mode === "ask" ? "active" : ""}
            onClick={() => onModeChange("ask")}
            type="button"
            aria-pressed={mode === "ask"}
          >
            Ask
          </button>
        </div>

        <label className="visually-hidden" htmlFor="command-box">
          Command
        </label>
        <textarea
          id="command-box"
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
              : disabled && disabledReason
                ? disabledReason
                : mode === "command"
                ? "Submit a command explicitly…"
                : "Describe what you want to do…"
          }
          readOnly={isRunning}
          disabled={disabled && !isRunning}
        />

        {isRunning ? (
          <button
            type="button"
            className="btn-stop"
            aria-label="Stop the command"
            onClick={() => onInterrupt?.()}
          >
            Stop
          </button>
        ) : (
          <button type="button" onClick={handleSubmit} disabled={cantSubmit}>
            {busy ? "Working…" : mode === "ask" ? "Draft plan" : "Run"}
          </button>
        )}
      </div>
    );
  },
);
