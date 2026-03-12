import { tauriInvoke } from "../../lib/tauriInvoke";
import type {
  PlannerGeneratePlanRequest,
  PlannerGeneratePlanResponse,
} from "@commandui/api-contract";

export function generatePlan(
  request: PlannerGeneratePlanRequest,
): Promise<PlannerGeneratePlanResponse> {
  return tauriInvoke("planner_generate_plan", { request });
}
