import type { PlannerStatus } from "@commandui/api-contract";
import { fontSizePercent } from "../lib/fontScale";
import { useModalDialog } from "../lib/useModalDialog";
import { PlannerStatusCard } from "./PlannerStatusCard";

type Props = {
  isOpen: boolean;
  onClose: () => void;
  productMode: "classic" | "guided";
  onProductModeChange: (mode: "classic" | "guided") => void;
  defaultInputMode: "command" | "ask";
  onDefaultInputModeChange: (mode: "command" | "ask") => void;
  fontSize: string;
  onFontSizeChange: (value: string) => void;
  simplifiedSummaries: boolean;
  onSimplifiedSummariesChange: (value: boolean) => void;
  plannerModel: string;
  onPlannerModelChange: (value: string) => void;
  plannerEndpoint: string;
  onPlannerEndpointChange: (value: string) => void;
  plannerStatus: PlannerStatus | null;
  onCheckPlanner: () => void;
};

export function SettingsDrawer({
  isOpen,
  onClose,
  productMode,
  onProductModeChange,
  defaultInputMode,
  onDefaultInputModeChange,
  fontSize,
  onFontSizeChange,
  simplifiedSummaries,
  onSimplifiedSummariesChange,
  plannerModel,
  onPlannerModelChange,
  plannerEndpoint,
  onPlannerEndpointChange,
  plannerStatus,
  onCheckPlanner,
}: Props) {
  const dialogRef = useModalDialog(isOpen, onClose);
  if (!isOpen) return null;
  const percent = fontSizePercent(fontSize);

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className="settings-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="drawer-header">
          <strong id="settings-title">Settings</strong>
          <button type="button" data-autofocus onClick={onClose}>
            Close
          </button>
        </div>

        <div className="settings-section">
          <div className="settings-field">
            <label htmlFor="settings-mode">Mode</label>
            <p className="settings-help" id="settings-mode-help">
              Classic hides the plan until there is one. Guided keeps the plan column open, and it also opens when Ask is not ready.
            </p>
            <select
              id="settings-mode"
              aria-describedby="settings-mode-help"
              value={productMode}
              onChange={(e) =>
                onProductModeChange(e.target.value as "classic" | "guided")
              }
            >
              <option value="classic">Classic</option>
              <option value="guided">Guided</option>
            </select>
          </div>

          <div className="settings-field">
            <label htmlFor="settings-input-mode">Default input mode</label>
            <p className="settings-help" id="settings-input-mode-help">
              Command runs what you type. Ask drafts a command for you to approve.
            </p>
            <select
              id="settings-input-mode"
              aria-describedby="settings-input-mode-help"
              value={defaultInputMode}
              onChange={(e) =>
                onDefaultInputModeChange(e.target.value as "command" | "ask")
              }
            >
              <option value="command">Command</option>
              <option value="ask">Ask</option>
            </select>
          </div>

          <div className="settings-field">
            <label htmlFor="settings-text-size">Text size</label>
            <p className="settings-help" id="settings-text-size-help">
              Scales the text and the terminal from 100% to 200%.
            </p>
            <div className="settings-scale">
              <input
                id="settings-text-size"
                type="range"
                min={100}
                max={200}
                step={10}
                value={percent}
                aria-describedby="settings-text-size-help"
                aria-valuetext={`${percent} percent`}
                onChange={(e) => onFontSizeChange(e.target.value)}
              />
              <span>{percent}%</span>
            </div>
          </div>

          <div className="settings-field">
            <label className="settings-check" htmlFor="settings-summaries">
              <input
                id="settings-summaries"
                type="checkbox"
                checked={simplifiedSummaries}
                aria-describedby="settings-summaries-help"
                onChange={(e) => onSimplifiedSummariesChange(e.target.checked)}
              />
              Simplified summaries
            </label>
            <p className="settings-help" id="settings-summaries-help">
              Uses only the first sentence of a drafted command's explanation.
            </p>
          </div>

          <div className="settings-field">
            <label htmlFor="settings-planner-model">Model</label>
            <p className="settings-help" id="settings-planner-model-help">
              The local model Ask uses to draft a command.
            </p>
            <input
              id="settings-planner-model"
              aria-describedby="settings-planner-model-help"
              value={plannerModel}
              onChange={(e) => onPlannerModelChange(e.target.value)}
            />
          </div>

          <div className="settings-field">
            <label htmlFor="settings-planner-endpoint">Where the model runs</label>
            <p className="settings-help" id="settings-planner-endpoint-help">
              The address of the model on this computer. The default is this computer.
            </p>
            <input
              id="settings-planner-endpoint"
              aria-describedby="settings-planner-endpoint-help"
              value={plannerEndpoint}
              onChange={(e) => onPlannerEndpointChange(e.target.value)}
            />
          </div>

          {plannerStatus && <PlannerStatusCard status={plannerStatus} onCheckAgain={onCheckPlanner} />}
          {!plannerStatus && (
            <button type="button" onClick={onCheckPlanner}>
              Check again
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
