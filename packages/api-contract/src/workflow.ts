import type { Workflow } from "@commandui/domain";

export type WorkflowAddRequest = {
  workflow: Workflow;
};

export type WorkflowAddResponse = {
  ok: boolean;
};

export type WorkflowListResponse = {
  workflows: Workflow[];
};
