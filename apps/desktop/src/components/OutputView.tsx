import { scrollRegionKeyDown } from "../lib/scrollRegion";
import { useModalDialog } from "../lib/useModalDialog";

export type OutputBlock = {
  id: string;
  command: string;
  headline: string;
  output: string;
};

type Props = {
  blocks: OutputBlock[];
  onClose: () => void;
};

export function OutputView({ blocks, onClose }: Props) {
  const ref = useModalDialog(true, onClose);
  return (
    <div className="history-overlay">
      <div
        ref={ref}
        className="history-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="output-title"
      >
        <div className="drawer-header">
          <strong id="output-title">Output</strong>
          <button type="button" data-autofocus onClick={onClose}>
            Close
          </button>
        </div>
        {blocks.length === 0 ? (
          <p>No commands in this session yet. Run one, and its output will be listed here.</p>
        ) : (
          <ol className="output-blocks">
            {blocks.map((block) => (
              <li key={block.id} className="output-block">
                <p className="output-command">{block.command || "(typed in the terminal)"}</p>
                <p>{block.headline}</p>
                {block.output && (
                  <pre
                    className="result-output"
                    tabIndex={0}
                    role="region"
                    aria-label={`Output of ${block.command || "a command typed in the terminal"}`}
                    onKeyDown={scrollRegionKeyDown}
                  >
                    {block.output}
                  </pre>
                )}
              </li>
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}
