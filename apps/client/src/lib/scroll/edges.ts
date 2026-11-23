export function hiddenEdges(scrollStart: number, clientSize: number, scrollSize: number): { start: boolean; end: boolean } {
  return { start: scrollStart > 0, end: scrollStart + clientSize < scrollSize - 1 };
}

/** Marks each edge with content past it, so the stylesheet fades that edge (MAT-003, MAT-004). */
export function edgeFade(body: HTMLElement): () => void {
  const mark = (): void => {
    const edges = hiddenEdges(body.scrollTop, body.clientHeight, body.scrollHeight);
    body.toggleAttribute('data-more-above', edges.start);
    body.toggleAttribute('data-more-below', edges.end);
  };
  mark();
  body.addEventListener('scroll', mark, { passive: true });
  const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(mark) : undefined;
  observer?.observe(body);
  return () => {
    body.removeEventListener('scroll', mark);
    observer?.disconnect();
  };
}
