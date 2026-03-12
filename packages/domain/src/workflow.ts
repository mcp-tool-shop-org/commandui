export type Workflow = {
  id: string;
  label: string;
  source: "raw" | "semantic";
  originalIntent?: string;
  command: string;
  projectRoot?: string;
  createdAt: string;
};
