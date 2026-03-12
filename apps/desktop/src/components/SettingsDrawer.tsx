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
  confirmMediumRisk: boolean;
  onConfirmMediumRiskChange: (value: boolean) => void;
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
  confirmMediumRisk,
  onConfirmMediumRiskChange,
}: Props) {
  if (!isOpen) return null;

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <strong>Settings</strong>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        <div className="settings-section">
          <div className="settings-row">
            <label>Mode</label>
            <select
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
            <label>Default Input Mode</label>
            <select
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

          <label className="settings-check">
            <input
              type="checkbox"
              checked={confirmMediumRisk}
              onChange={(e) => onConfirmMediumRiskChange(e.target.checked)}
            />
            Confirm medium-risk commands
          </label>
        </div>
      </div>
    </div>
  );
}
