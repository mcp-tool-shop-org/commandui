/** A word next to a status dot, so the state is not colour alone. */
export function runStatusLabel(status: string): string {
  switch (status) {
    case "success":
      return "Finished";
    case "failed":
      return "Did not work";
    case "running":
      return "Running";
    case "interrupted":
      return "Stopped";
    case "skipped":
      return "Skipped";
    default:
      return "Waiting";
  }
}
