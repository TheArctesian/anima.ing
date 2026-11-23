/** jsdom has no pointer capture. */
export function stubPointerCapture(): number[] {
  const captured: number[] = [];
  Object.defineProperty(Element.prototype, 'setPointerCapture', {
    configurable: true,
    value: (pointerId: number) => captured.push(pointerId),
  });
  return captured;
}

export function pointer(
  type: 'pointerdown' | 'pointermove' | 'pointerup' | 'pointercancel',
  at: number,
  clientY: number,
  clientX = 0,
): PointerEvent {
  const event = new MouseEvent(type, { bubbles: true, button: 0, clientX, clientY }) as unknown as PointerEvent;
  Object.defineProperty(event, 'timeStamp', { value: at });
  Object.defineProperty(event, 'pointerId', { value: 1 });
  return event;
}
