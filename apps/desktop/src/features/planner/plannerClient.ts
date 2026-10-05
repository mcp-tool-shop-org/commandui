import { tauriInvoke } from "../../lib/tauriInvoke";
import type {
  PlannerGeneratePlanRequest,
  PlannerGeneratePlanResponse,
} from "@commandui/api-contract";

/**
 * The planner waits up to 90 s for the model (OllamaConfig.timeout_secs), so
 * the first Ask after CommandUI opens can load the model. The screen waits a
 * little longer than that, so the planner's own answer always arrives first.
 */
export const PLAN_TIMEOUT_MS = 100_000;

export const PLAN_TIMEOUT_MESSAGE =
  "The model did not answer in time. It may still be loading. Try again in a moment, or check the model in Settings.";

export function generatePlan(
  request: PlannerGeneratePlanRequest,
): Promise<PlannerGeneratePlanResponse> {
  return tauriInvoke(
    "planner_generate_plan",
    { request },
    { timeoutMs: PLAN_TIMEOUT_MS, timeoutMessage: PLAN_TIMEOUT_MESSAGE },
  );
}
