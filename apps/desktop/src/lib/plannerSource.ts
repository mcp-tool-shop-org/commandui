/**
 * The planner reports which engine wrote the plan. History stores that
 * value. Anything else is left unset rather than recorded as a guess.
 */
export function recordedPlannerSource(reported: string): "ollama" | "mock" | undefined {
  if (reported === "ollama" || reported === "mock") return reported;
  return undefined;
}
