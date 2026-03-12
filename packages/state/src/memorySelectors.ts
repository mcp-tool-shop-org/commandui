import type { MemoryItem } from "@commandui/domain";

export function resolveEffectiveMemory(
  items: MemoryItem[],
  projectRoot?: string,
): MemoryItem[] {
  const projectItems = items.filter(
    (item) => item.scope === "project" && item.projectRoot === projectRoot,
  );

  const projectKeys = new Set(
    projectItems.map((item) => `${item.kind}:${item.key}`),
  );

  const globalItems = items.filter(
    (item) =>
      item.scope === "global" &&
      !projectKeys.has(`${item.kind}:${item.key}`),
  );

  return [...projectItems, ...globalItems];
}
