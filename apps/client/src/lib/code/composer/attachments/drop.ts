import type { Attachment } from 'svelte/attachments';

/** The type a browser lists for a drag that carries files of the system. */
const FILES = 'Files';

/** 03 B.2.6: a drag counts when it carries files, or one of the composer's own types (a file of the tree, a pull request). */
export function dragCounts(types: readonly string[], own: readonly string[] = []): boolean {
  return types.includes(FILES) || own.some((type) => types.includes(type));
}

export type DragStep = 'enter' | 'over' | 'leave' | 'end';

/**
 * How many elements of the zone the drag is over after a step. An enter of a child comes before the leave of its
 * parent, so the count stays above zero while the drag is inside and the state does not flicker. T3 reads
 * `relatedTarget` for this; our web view (WebKit) leaves it null on a drag.
 */
export function dragDepth(depth: number, step: DragStep): number {
  if (step === 'enter') return depth + 1;
  if (step === 'leave') return Math.max(0, depth - 1);
  // A drag that began inside, or whose enter was missed, still shows on the first move.
  if (step === 'over') return Math.max(1, depth);
  return 0;
}

interface Transfer {
  readonly files: Iterable<File>;
  readonly items?: Iterable<DataTransferItem> | undefined;
}

/** The files and the folders of a drop or a paste; a browser that cannot tell gives every entry as a file. */
export function splitDropped(transfer: Transfer): { files: File[]; folders: File[] } {
  if (transfer.items === undefined) return { files: [...transfer.files], folders: [] };
  const files: File[] = [];
  const folders: File[] = [];
  for (const entry of transfer.items) {
    const file = entry.kind === 'file' ? entry.getAsFile() : null;
    if (file === null) continue;
    const listed = typeof entry.webkitGetAsEntry === 'function' ? entry.webkitGetAsEntry() : null;
    (listed?.isDirectory === true ? folders : files).push(file);
  }
  // A web view can list a copied file only in `files`: it is not lost.
  if (files.length + folders.length === 0) return { files: [...transfer.files], folders: [] };
  return { files, folders };
}

export interface DropZone {
  /** The drag-over state changed: the card shows or hides its fill and line (03 B.2.6). */
  onOver: (over: boolean) => void;
  /** Files of the system were dropped. A drop of one of the `own` types is left to the composer's own listener. */
  onFiles: (files: readonly File[], folders: readonly File[]) => void;
  /** The composer's own drag types that also light the card. */
  own?: readonly string[];
}

/**
 * Makes an element a drop zone for files: `{@attach dropZone({ onOver, onFiles })}`. The state also clears when a
 * drag ends anywhere (Escape cancels a drag without a leave).
 */
export function dropZone(zone: DropZone): Attachment<HTMLElement> {
  return (element) => {
    let depth = 0;
    const move = (step: DragStep): void => {
      const before = depth > 0;
      depth = dragDepth(depth, step);
      if (depth > 0 !== before) zone.onOver(depth > 0);
    };
    const claimed = (event: DragEvent): boolean => {
      const transfer = event.dataTransfer;
      if (transfer === null || !dragCounts(transfer.types, zone.own)) return false;
      event.preventDefault();
      return true;
    };
    const steps: Record<string, (event: DragEvent) => void> = {
      dragenter: (event) => {
        if (claimed(event)) move('enter');
      },
      dragover: (event) => {
        if (!claimed(event) || event.dataTransfer === null) return;
        if (event.dataTransfer.types.includes(FILES)) event.dataTransfer.dropEffect = 'copy';
        move('over');
      },
      dragleave: (event) => {
        if (claimed(event)) move('leave');
      },
      drop: (event) => {
        if (!claimed(event) || event.dataTransfer === null) return;
        move('end');
        const { files, folders } = splitDropped(event.dataTransfer);
        if (files.length > 0 || folders.length > 0) zone.onFiles(files, folders);
      },
    };
    const ended = (): void => {
      move('end');
    };
    for (const [name, step] of Object.entries(steps)) element.addEventListener(name, step as EventListener);
    window.addEventListener('dragend', ended);
    window.addEventListener('drop', ended);
    return () => {
      for (const [name, step] of Object.entries(steps)) element.removeEventListener(name, step as EventListener);
      window.removeEventListener('dragend', ended);
      window.removeEventListener('drop', ended);
      if (depth > 0) zone.onOver(false);
    };
  };
}
