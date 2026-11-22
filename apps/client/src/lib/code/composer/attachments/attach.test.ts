import { describe, expect, it } from 'vitest';
import { admit, attachmentOf, classify, fileIcon, foldedName, laidOut, LIMITS, pasteDecision, previewStep, readingWords, sizeWords, type ClipboardFacts, type DraftFacts, type FileFacts } from './attach';
import { ATTACHMENTS } from './fixture';

const MB = 1024 * 1024;

function file(name: string, type = '', size = 10): FileFacts {
  return { name, type, size };
}

describe('the class of a file', () => {
  it.each([
    ['photo.png', 'image/png', { kind: 'image', mime: 'image/png' }],
    ['photo.JPG', '', { kind: 'image', mime: 'image/jpeg' }],
    ['photo.webp', 'application/octet-stream', { kind: 'image', mime: 'image/webp' }],
    ['shot.heic', '', { kind: 'image', mime: '' }],
    ['shot.heif', 'image/heif', { kind: 'image', mime: 'image/heif' }],
    ['logo.svg', 'image/svg+xml', { kind: 'file', mime: 'image/svg+xml' }],
    ['logo.svg', '', { kind: 'file', mime: 'image/svg+xml' }],
    ['raw.dng', 'image/tiff', { kind: 'file', mime: 'image/x-adobe-dng' }],
    ['raw', 'image/x-dng', { kind: 'file', mime: 'image/x-adobe-dng' }],
    ['scan.tiff', 'image/tiff', { kind: 'file', mime: 'image/tiff' }],
    ['layers.psd', 'image/vnd.adobe.photoshop', { kind: 'unsupported' }],
    ['clip.mov', '', { kind: 'video', mime: 'video/quicktime' }],
    ['clip', 'video/webm;codecs=vp9', { kind: 'video', mime: 'video/webm' }],
    ['main.ts', 'video/mp2t', { kind: 'file', mime: 'video/mp2t' }],
    ['notes.txt', 'text/plain;charset=utf-8', { kind: 'text', mime: 'text/plain;charset=utf-8' }],
    ['report.pdf', 'application/pdf', { kind: 'file', mime: 'application/pdf' }],
    ['Makefile', '', { kind: 'file', mime: 'application/octet-stream' }],
    ['.png', '', { kind: 'file', mime: 'application/octet-stream' }],
  ])('%s (%s)', (name, type, expected) => {
    expect(classify(file(name, type))).toEqual(expected);
  });
});

describe('what the draft takes', () => {
  it('takes images and files and keeps the file object of each', () => {
    const png = file('a.png', 'image/png');
    const pdf = file('b.pdf', 'application/pdf');
    expect(admit([], [png, pdf])).toEqual({
      taken: [
        { file: png, kind: 'image', mime: 'image/png' },
        { file: pdf, kind: 'file', mime: 'application/pdf' },
      ],
      refused: [],
    });
  });

  it('refuses with the words of each limit', () => {
    const { taken, refused } = admit([], [
      file('layers.psd', 'image/vnd.adobe.photoshop'),
      file('empty.txt', 'text/plain', 0),
      file('big.zip', 'application/zip', 50 * MB + 1),
      file('huge.png', 'image/png', 10 * MB + 1),
      file('edge.zip', 'application/zip', 50 * MB),
    ]);
    expect(taken.map((one) => one.file.name)).toEqual(['edge.zip']);
    expect(refused).toEqual([
      { name: 'layers.psd', words: "'layers.psd' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images." },
      { name: 'empty.txt', words: "'empty.txt' is empty or could not be read." },
      { name: 'big.zip', words: "'big.zip' exceeds the 50 MB attachment limit." },
      { name: 'huge.png', words: "'huge.png' is too large to attach." },
    ]);
  });

  it('counts every attachment of the draft, also one still being read, and stops at the limit', () => {
    const limits = { ...LIMITS, count: ATTACHMENTS.length + 1 };
    const { taken, refused } = admit(ATTACHMENTS, [file('a.png', 'image/png'), file('b.png', 'image/png')], limits);
    expect(taken).toHaveLength(1);
    expect(refused).toEqual([{ name: 'b.png', words: `You can attach up to ${String(limits.count)} files per message.` }]);
  });

  it('follows the limit of the server, and takes only images when the server takes no files', () => {
    expect(admit([], [file('a.bin', '', 1501)], { ...LIMITS, fileBytes: 1500 }).refused[0]?.words).toBe("'a.bin' exceeds the 1500 bytes attachment limit.");
    expect(admit([], [file('a.bin', '', 2049)], { ...LIMITS, fileBytes: 2048 }).refused[0]?.words).toBe("'a.bin' exceeds the 2 KB attachment limit.");
    const none = admit([], [file('a.pdf', 'application/pdf'), file('a.png', 'image/png')], { ...LIMITS, fileBytes: null });
    expect(none.taken.map((one) => one.file.name)).toEqual(['a.png']);
    expect(none.refused[0]?.words).toBe('This server does not support file attachments.');
  });

  it('makes the attachment of a taken file, with a name for a file that has none', () => {
    const [taken] = admit([], [file('', 'image/png', 2048)]).taken;
    if (taken === undefined) throw new Error('not taken');
    expect(attachmentOf(taken, 'id-1', 'blob:x')).toEqual({ id: 'id-1', name: 'image', kind: 'image', size: 2048, mime: 'image/png', preview: 'blob:x', state: { state: 'ready' } });
  });
});

describe('the words of a size', () => {
  it('rounds kilobytes up and turns to megabytes after the rounding', () => {
    expect(sizeWords(0)).toBe('1 KB');
    expect(sizeWords(1025)).toBe('2 KB');
    expect(sizeWords(1023 * 1024)).toBe('1023 KB');
    expect(sizeWords(1023.5 * 1024)).toBe('1.0 MB');
    expect(sizeWords(1_572_864)).toBe('1.5 MB');
  });
});

describe('a paste', () => {
  const clip: ClipboardFacts = { files: [], folders: 0, plainText: '', asText: false, chips: false };
  const draft: DraftFacts = { promptLength: 0, selection: { start: 0, end: 0 }, names: [], room: true };
  const long = 'x'.repeat(32 * 1024);

  it('attaches an image always, and a plain file only when no text came with it', () => {
    const png = file('a.png', 'image/png');
    const pdf = file('a.pdf', 'application/pdf');
    expect(pasteDecision({ ...clip, files: [png], plainText: 'a.png' }, draft)).toEqual({ act: 'attach', folders: false });
    expect(pasteDecision({ ...clip, files: [pdf] }, draft)).toEqual({ act: 'attach', folders: false });
    expect(pasteDecision({ ...clip, files: [pdf], plainText: 'cell A1' }, draft)).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, files: [file('a.psd', 'image/vnd.adobe.photoshop')], plainText: 'a' }, draft)).toEqual({ act: 'attach', folders: false });
  });

  it('leaves the files when the paste is "as text" with text, or holds chips of a composer; folders always count', () => {
    const png = file('a.png', 'image/png');
    expect(pasteDecision({ ...clip, files: [png], plainText: 'words', asText: true }, draft)).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, files: [png], asText: true }, draft)).toEqual({ act: 'attach', folders: false });
    expect(pasteDecision({ ...clip, files: [png], chips: true }, draft)).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, folders: 1, chips: true }, draft)).toEqual({ act: 'attach', folders: true });
  });

  it('folds a text of 32 KB, by characters or by bytes, into a file with a free name', () => {
    expect(pasteDecision({ ...clip, plainText: long.slice(1) }, draft)).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, plainText: long }, draft)).toEqual({ act: 'fold', name: 'pasted-text.txt', mime: 'text/plain;charset=utf-8' });
    expect(pasteDecision({ ...clip, plainText: 'é'.repeat(16 * 1024) }, { ...draft, names: ['Pasted-Text.txt', 'pasted-text-2.txt'] })).toEqual({
      act: 'fold',
      name: 'pasted-text-3.txt',
      mime: 'text/plain;charset=utf-8',
    });
    expect(pasteDecision({ ...clip, plainText: long, asText: true }, draft)).toEqual({ act: 'editor' });
  });

  it('folds a short text that would break the limit of the prompt, counting what the selection gives back', () => {
    const full = { ...draft, promptLength: 120_000 };
    expect(pasteDecision({ ...clip, plainText: 'abc' }, full).act).toBe('fold');
    expect(pasteDecision({ ...clip, plainText: 'abc' }, { ...full, selection: { start: 10, end: 13 } })).toEqual({ act: 'editor' });
  });

  it('pastes inline when it cannot fold, and refuses only what would break the prompt', () => {
    expect(pasteDecision({ ...clip, plainText: long }, { ...draft, room: false })).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, plainText: long }, draft, { ...LIMITS, fileBytes: null })).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, plainText: long }, draft, { ...LIMITS, fileBytes: 1024 })).toEqual({ act: 'editor' });
    expect(pasteDecision({ ...clip, plainText: 'abc' }, { ...draft, promptLength: 120_000, room: false })).toEqual({
      act: 'refuse',
      title: 'Pasted text is too large for this message',
      description: 'Remove some text or an attachment, then paste again.',
    });
    expect(pasteDecision({ ...clip, plainText: long }, { ...draft, promptLength: 120_000 }, { ...LIMITS, fileBytes: 1024 })).toEqual({
      act: 'refuse',
      title: 'Pasted text is too large to attach',
      description: 'Reduce the clipboard contents or save a smaller excerpt as a file.',
    });
  });

  it('names folded pastes in a row', () => {
    expect(foldedName([])).toBe('pasted-text.txt');
    expect(foldedName(['pasted-text.txt'])).toBe('pasted-text-2.txt');
  });
});

describe('where an attachment shows', () => {
  it('puts images and videos in the media row and the rest in the list, less the files the text shows as chips', () => {
    const { media, files } = laidOut(ATTACHMENTS, new Set(['a-pdf', 'a-image']));
    expect(media.map((one) => one.id)).toEqual(['a-image', 'a-reading', 'a-failed', 'a-video']);
    expect(files.map((one) => one.id)).toEqual(['a-text', 'a-code', 'a-lost']);
  });

  it('gives each row of the list an icon for its sort', () => {
    expect(ATTACHMENTS.map(fileIcon)).toEqual(['image', 'image', 'image', 'video', 'file-text', 'file', 'file-code', 'file-text']);
  });

  it('steps through the previews and goes round at the ends', () => {
    const shown = ATTACHMENTS.slice(0, 3);
    expect(previewStep(shown, 'a-image', -1)).toBe('a-failed');
    expect(previewStep(shown, 'a-failed', 1)).toBe('a-image');
    expect(previewStep(shown.slice(0, 1), 'a-image', 1)).toBeNull();
    expect(previewStep(shown, 'gone', 1)).toBeNull();
  });
});

describe('the limits of the files of one message', () => {
  it('refuses the image that brings the images of the message over their limit together, and takes the next that fits', () => {
    const { taken, refused } = admit([], [file('a.png', 'image/png', 6 * MB), file('b.png', 'image/png', 6 * MB), file('c.png', 'image/png', 3 * MB), file('notes.txt', 'text/plain', 6 * MB)], { ...LIMITS, imagesBytes: 10 * MB });
    expect(taken.map((one) => one.file.name)).toEqual(['a.png', 'c.png', 'notes.txt']);
    expect(refused).toEqual([{ name: 'b.png', words: "'b.png' does not fit: the images of one message can be 10 MB together." }]);
  });

  it('says how far a file is while the runner takes it', () => {
    expect(readingWords({ size: 200, state: { state: 'reading', sent: 50 } })).toBe('25%');
    expect(readingWords({ size: 200, state: { state: 'reading', sent: 200 } })).toBe('99%');
    expect(readingWords({ size: 200, state: { state: 'reading' } })).toBe('Reading');
  });
});
