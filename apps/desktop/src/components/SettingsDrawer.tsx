import type { PlannerStatus } from "@commandui/api-contract";
import { useModalDialog } from "../lib/useModalDialog";
import { PlannerStatusCard } from "./PlannerStatusCard";

type Props = {
  isOpen: boolean;
  onClose: () => void;
  productMode: "classic" | "guided";
  onProductModeChange: (mode: "classic" | "guided") => void;
  defaultInputMode: "command" | "ask";
  onDefaultInputModeChange: (mode: "command" | "ask") => void;
  reducedClutter: boolean;
  onReducedClutterChange: (value: boolean) => void;
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
  reducedClutter,
  onReducedClutterChange,
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
          <div className="settings-row">
            <label htmlFor="settings-mode">Mode</label>
            <select
              id="settings-mode"
              value={productMode}
              onChange={(e) =>
                onProductModeChange(e.target.value as "classic" | "guided")
              }
            >
              <option value="classic">Classic</option>
              <option value="guided">Guided</option>
            </select>
          </div>

          <div className="settings-row">
            <label htmlFor="settings-input-mode">Default Input Mode</label>
            <select
              id="settings-input-mode"
              value={defaultInputMode}
              onChange={(e) =>
                onDefaultInputModeChange(e.target.value as "command" | "ask")
              }
            >
              <option value="command">Command</option>
              <option value="ask">Ask</option>
            </select>
          </div>

          <label className="settings-check">
            <input
              type="checkbox"
              checked={reducedClutter}
              onChange={(e) => onReducedClutterChange(e.target.checked)}
            />
            Reduced clutter
          </label>

          <label className="settings-check">
            <input
              type="checkbox"
              checked={simplifiedSummaries}
              onChange={(e) => onSimplifiedSummariesChange(e.target.checked)}
            />
            Simplified summaries
          </label>

          <div className="settings-row">
            <label htmlFor="settings-planner-model">Model</label>
            <input
              id="settings-planner-model"
              value={plannerModel}
              onChange={(e) => onPlannerModelChange(e.target.value)}
            />
          </div>

          <div className="settings-row">
            <label htmlFor="settings-planner-endpoint">Where the model runs</label>
            <input
              id="settings-planner-endpoint"
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
