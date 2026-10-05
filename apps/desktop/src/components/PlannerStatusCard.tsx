import type { PlannerStatus } from "@commandui/api-contract";

type Props = {
  status: PlannerStatus;
  onCheckAgain?: () => void;
};

export function PlannerStatusCard({ status, onCheckAgain }: Props) {
  return (
    <div className="planner-status">
      <p className="planner-status-headline">{status.headline}</p>
      <p>{status.fix}</p>
      <p>
        <a href={status.link}>{status.linkLabel}</a>
      </p>
      {onCheckAgain && (
        <button type="button" onClick={onCheckAgain}>
          Check again
        </button>
      )}
    </div>
  );
}
