type ScrollKeyEvent = {
  key: string;
  preventDefault: () => void;
  currentTarget: HTMLElement;
};

/**
 * Home, End, and the arrow keys scroll a focused output region. A focused
 * `<pre>` does not do this on its own, so a keyboard user could not read a
 * long block.
 */
export function scrollRegionKeyDown(event: ScrollKeyEvent): void {
  const el = event.currentTarget;
  const page = Math.max(el.clientHeight, 24);
  let next = el.scrollTop;
  switch (event.key) {
    case "ArrowDown":
      next += 24;
      break;
    case "ArrowUp":
      next -= 24;
      break;
    case "PageDown":
      next += page;
      break;
    case "PageUp":
      next -= page;
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = el.scrollHeight;
      break;
    default:
      return;
  }
  event.preventDefault();
  el.scrollTop = next;
}
