import type { CommandResult, ResultAction } from "../lib/commandResult";
import { collapseRedraws, resultText } from "../lib/commandResult";

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
    <div className="result-line" data-cause={result.cause} data-testid="result-line">
      <p className="result-line-text" role={result.announce ? "status" : undefined}>
        {text}
      </p>
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
        <pre className="result-output">{collapseRedraws(outputText)}</pre>
      )}
    </div>
  );
}
