// jsdom has no 2D canvas, and every Guacamole display layer asks for one.

function blankContext(): object {
  const written = new Map<PropertyKey, unknown>();
  return new Proxy(
    {},
    {
      get: (_target, key) => (written.has(key) ? written.get(key) : () => undefined),
      set: (_target, key, value) => {
        written.set(key, value);
        return true;
      },
    },
  );
}

export function installBlankCanvas(): () => void {
  const prototype = HTMLCanvasElement.prototype;
  const original = Object.getOwnPropertyDescriptor(prototype, 'getContext');
  Object.defineProperty(prototype, 'getContext', { configurable: true, writable: true, value: blankContext });
  return () => {
    if (original !== undefined) Object.defineProperty(prototype, 'getContext', original);
  };
}
