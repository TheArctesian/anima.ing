export type VisibleArea = Pick<VisualViewport, 'offsetTop' | 'height' | 'addEventListener' | 'removeEventListener'>;

/** iOS moves the visible area when the keyboard opens and a fixed element stays behind, so `bar.css` follows this (MOB-011). */
export function followVisualTop(root: HTMLElement, area: VisibleArea | null | undefined): () => void {
  if (area == null) return () => undefined;
  const update = (): void => {
    root.style.setProperty('--visual-block-start', `${String(Math.max(0, area.offsetTop))}px`);
    root.style.setProperty('--visual-block-size', `${String(area.height)}px`);
  };
  update();
  area.addEventListener('resize', update);
  area.addEventListener('scroll', update);
  return () => {
    area.removeEventListener('resize', update);
    area.removeEventListener('scroll', update);
  };
}

