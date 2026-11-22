// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { dragCounts, dragDepth, dropZone, splitDropped } from './drop';

const OWN = 'application/x-anima-mention';

function drag(name: string, types: readonly string[], files: File[] = []): Event {
  const event = new Event(name, { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'dataTransfer', { value: { types, files, dropEffect: 'none' } });
  return event;
}

function zone(): { card: HTMLElement; child: HTMLElement; onOver: ReturnType<typeof vi.fn>; onFiles: ReturnType<typeof vi.fn>; off: () => void } {
  const card = document.createElement('div');
  const child = card.appendChild(document.createElement('p'));
  document.body.append(card);
  const onOver = vi.fn();
  const onFiles = vi.fn();
  const off = dropZone({ onOver, onFiles, own: [OWN] })(card);
  return { card, child, onOver, onFiles, off: typeof off === 'function' ? off : () => undefined };
}

describe('which drags count', () => {
  it('counts files and the composer own types, not text or a link', () => {
    expect(dragCounts(['Files'])).toBe(true);
    expect(dragCounts(['text/plain', OWN], [OWN])).toBe(true);
    expect(dragCounts(['text/plain', 'text/uri-list'], [OWN])).toBe(false);
  });

  it('keeps the depth above zero while the drag moves between the children of the zone', () => {
    let depth = dragDepth(0, 'enter');
    depth = dragDepth(depth, 'enter');
    depth = dragDepth(depth, 'leave');
    expect(depth).toBe(1);
    expect(dragDepth(depth, 'leave')).toBe(0);
    expect(dragDepth(0, 'leave')).toBe(0);
    expect(dragDepth(0, 'over')).toBe(1);
    expect(dragDepth(3, 'over')).toBe(3);
    expect(dragDepth(3, 'end')).toBe(0);
  });
});

describe('the drop zone', () => {
  it('says "over" one time while a file drag crosses its children, and "not over" when it leaves', () => {
    const { card, child, onOver, off } = zone();
    const enter = drag('dragenter', ['Files']);
    card.dispatchEvent(enter);
    child.dispatchEvent(drag('dragenter', ['Files']));
    card.dispatchEvent(drag('dragleave', ['Files']));
    expect(enter.defaultPrevented).toBe(true);
    expect(onOver.mock.calls).toEqual([[true]]);
    child.dispatchEvent(drag('dragleave', ['Files']));
    expect(onOver.mock.calls).toEqual([[true], [false]]);
    off();
  });

  it('leaves a drag of text alone', () => {
    const { card, onOver, off } = zone();
    const enter = drag('dragenter', ['text/plain']);
    card.dispatchEvent(enter);
    expect(enter.defaultPrevented).toBe(false);
    expect(onOver).not.toHaveBeenCalled();
    off();
  });

  it('gives the dropped files and clears the state', () => {
    const { card, onOver, onFiles, off } = zone();
    const dropped = new File(['a'], 'a.txt');
    const over = drag('dragover', ['Files']);
    card.dispatchEvent(over);
    card.dispatchEvent(drag('drop', ['Files'], [dropped]));
    expect((over as DragEvent).dataTransfer?.dropEffect).toBe('copy');
    expect(onFiles.mock.calls).toEqual([[[dropped], []]]);
    expect(onOver.mock.calls).toEqual([[true], [false]]);
    off();
  });

  it('lights for a drag of its own type and leaves that drop to the composer', () => {
    const { card, onOver, onFiles, off } = zone();
    card.dispatchEvent(drag('dragenter', [OWN]));
    card.dispatchEvent(drag('drop', [OWN]));
    expect(onOver.mock.calls).toEqual([[true], [false]]);
    expect(onFiles).not.toHaveBeenCalled();
    off();
  });

  it('clears when the drag ends elsewhere, and when the zone goes away', () => {
    const { card, onOver, off } = zone();
    card.dispatchEvent(drag('dragenter', ['Files']));
    window.dispatchEvent(new Event('dragend'));
    card.dispatchEvent(drag('dragenter', ['Files']));
    off();
    expect(onOver.mock.calls).toEqual([[true], [false], [true], [false]]);
  });
});

describe('what was dropped', () => {
  it('tells folders from files where the browser can', () => {
    const one = new File(['a'], 'a.txt');
    const folder = new File([], 'src');
    const entry = (file: File, isDirectory: boolean, kind = 'file'): DataTransferItem => ({ kind, getAsFile: () => file, webkitGetAsEntry: () => ({ isDirectory }) }) as unknown as DataTransferItem;
    expect(splitDropped({ files: [one, folder], items: [entry(one, false), entry(folder, true), entry(one, false, 'string')] })).toEqual({ files: [one], folders: [folder] });
    expect(splitDropped({ files: [one, folder] })).toEqual({ files: [one, folder], folders: [] });
    // A web view that lists a copied file only in `files`, or that cannot tell a folder: the file is not lost.
    expect(splitDropped({ files: [one], items: [entry(one, false, 'string')] })).toEqual({ files: [one], folders: [] });
    const plain = { kind: 'file', getAsFile: () => one } as unknown as DataTransferItem;
    expect(splitDropped({ files: [one], items: [plain] })).toEqual({ files: [one], folders: [] });
  });
});
