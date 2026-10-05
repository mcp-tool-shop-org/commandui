import type { CommandResult, ResultAction } from "../lib/commandResult";
import { collapseRedraws, resultText } from "../lib/commandResult";
import { scrollRegionKeyDown } from "../lib/scrollRegion";

const ACTION_LABEL: Record<ResultAction, string> = {
  "show-output": "Show output",
  "ask-fix": "Ask how to fix it",
  "run-again": "Run again",
  "ask-instead": "Ask CommandUI instead",
  stop: "Stop",
};

type Props = {
  result: CommandResult | null;
  outputOpen?: boolean;
  outputText?: string;
  onAction?: (action: ResultAction) => void;
};

export function ResultLine({ result, outputOpen = false, outputText = "", onAction }: Props) {
  if (!result) return null;
  const text = resultText(result);
  return (
    <div
      id="result-line"
      className="result-line"
      data-cause={result.cause}
      data-testid="result-line"
      tabIndex={0}
      role="region"
      aria-label="Command result"
    >
      <p className="result-line-text">{text}</p>
      {result.actions.length > 0 && (
        <div className="result-line-actions">
          {result.actions.map((action) => (
            <button
              key={action}
              type="button"
              aria-pressed={action === "show-output" ? outputOpen : undefined}
              onClick={() => onAction?.(action)}
            >
              {ACTION_LABEL[action]}
            </button>
          ))}
        </div>
      )}
      {outputOpen && outputText && (
        <pre
          className="result-output"
          tabIndex={0}
          role="region"
          aria-label="Command output"
          onKeyDown={scrollRegionKeyDown}
        >
          {collapseRedraws(outputText)}
        </pre>
      )}
    </div>
  );
}
