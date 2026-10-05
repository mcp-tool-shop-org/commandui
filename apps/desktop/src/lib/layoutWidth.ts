/** Layout width, in CSS pixels, under which the main screen uses one column. */
export const NARROW_LAYOUT_PX = 960;

/**
 * The shell's layout width. Zoom can make clientWidth and the border box
 * disagree. The smaller positive one is the space the columns share.
 * Zero means the box has not been laid out yet.
 */
export function layoutWidth(layoutPx: number, visualPx: number): number {
  const layout = Number.isFinite(layoutPx) && layoutPx > 0 ? layoutPx : 0;
  const visual = Number.isFinite(visualPx) && visualPx > 0 ? visualPx : 0;
  if (layout && visual) return Math.min(layout, visual);
  return layout || visual;
}

/** True when the measured layout box is too narrow for the side-by-side plan. */
export function layoutIsNarrow(width: number): boolean {
  return width > 0 && width < NARROW_LAYOUT_PX;
}
