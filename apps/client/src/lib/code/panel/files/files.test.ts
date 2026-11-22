import { describe, expect, it } from 'vitest';
import { crumbsFor, folderMenu, folderMenuIntent, parentPath, type FolderListing } from './crumbs';
import { EXPLORER_DEFAULT, explorerMax, explorerWidth } from './explorer';
import {
  fileKind, FILE_TEXT_PREVIEW_MAX_BYTES, limitNotice, rangeLabel, READ_ERRORS, renderToggle, showsRendered, toggleFace, toolbarShows, unsupportedWords,
  viewerFor,
} from './kinds';
import { fileRowMenu, ROW_MENU_PROBLEMS, rowMenuEditor } from './row-menu';
import { centredTop, clampLine, sourceLines, withoutFrontMatter } from './viewers/lines';
import { fitScale, percent, sizeLabel, zoomIn, zoomOut, ZOOM_MAX, ZOOM_MIN } from './viewers/zoom';

const labels = (rows: ReturnType<typeof folderMenu>): string[] => rows.map((row) => (row.kind === 'line' ? '-' : row.label));

describe('the kind of a file', () => {
  it('comes from the extension, in any case, and is text with none', () => {
    expect(fileKind('docs/README.MD')).toBe('markdown');
    expect(fileKind('a/flow.mmd')).toBe('mermaid');
    expect(fileKind('data.tsv')).toBe('table');
    expect(fileKind('index.htm')).toBe('html');
    expect(fileKind('logo.webp')).toBe('image');
    expect(fileKind('paper.pdf')).toBe('pdf');
    expect(fileKind('build.tar.gz')).toBe('archive');
    expect(fileKind('app.sqlite3')).toBe('sqlite');
    expect(fileKind('plan.docx')).toBe('office');
    expect(fileKind('run.ipynb')).toBe('notebook');
    expect(fileKind('face.woff2')).toBe('font');
    expect(fileKind('src/main.rs')).toBe('text');
    expect(fileKind('some.dir/Makefile')).toBe('text');
  });
});

describe('the viewer of a file', () => {
  it('follows the order: image, kinds with no text, error, loading, rendered, source', () => {
    expect(viewerFor({ path: 'a.png', failed: true })).toBe('image');
    expect(viewerFor({ path: 'a.pdf', failed: true })).toBe('unsupported');
    expect(viewerFor({ path: 'a.ts', binary: true })).toBe('unsupported');
    expect(viewerFor({ path: 'a.ts', attachment: true })).toBe('unsupported');
    expect(viewerFor({ path: 'a.ts', failed: true, loading: true })).toBe('error');
    expect(viewerFor({ path: 'a.md', loading: true, rendered: true })).toBe('loading');
    expect(viewerFor({ path: 'a.md', rendered: true })).toBe('markdown');
    expect(viewerFor({ path: 'a.md' })).toBe('source');
    expect(viewerFor({ path: 'a.html', rendered: true })).toBe('source');
  });

  it('names the kind that it cannot show', () => {
    expect(unsupportedWords({ path: 'a.zip' })).toBe('Archive files cannot be shown here yet.');
    expect(unsupportedWords({ path: 'a.bin', binary: true })).toBe('This file is binary and cannot be shown here yet.');
  });
});

describe('the rendered / source toggle', () => {
  it('has the words and the first view of each kind', () => {
    expect(renderToggle('markdown')).toEqual({ toSource: 'Show markdown source', toRendered: 'Show rendered markdown', renderedByDefault: false });
    expect(renderToggle('mermaid')).toEqual({ toSource: 'Show Mermaid source', toRendered: 'Show rendered diagram', renderedByDefault: true });
    expect(renderToggle('table')).toEqual({ toSource: 'Show source', toRendered: 'Show table', renderedByDefault: true });
    expect(renderToggle('html')).toEqual({ toSource: 'Show HTML source', toRendered: 'Show rendered page', renderedByDefault: true });
    expect(renderToggle('text')).toBeUndefined();
    expect(toggleFace('table', false)).toEqual({ label: 'Show table', icon: 'table-2' });
    expect(toggleFace('markdown', true)).toEqual({ label: 'Show markdown source', icon: 'code-xml' });
    expect(toggleFace('markdown', false)).toEqual({ label: 'Show rendered markdown', icon: 'eye' });
  });

  it('shows the source of a file opened at a line until the toggle is pressed', () => {
    expect(showsRendered('markdown')).toBe(false);
    expect(showsRendered('markdown', { kept: true })).toBe(true);
    expect(showsRendered('markdown', { kept: true, atLine: true })).toBe(false);
    expect(showsRendered('markdown', { kept: true, atLine: true, pressed: true })).toBe(true);
    expect(showsRendered('text', { kept: true })).toBe(false);
  });
});

describe('the toolbar of a file', () => {
  it('shows each control only when it has a use', () => {
    expect(toolbarShows({ path: 'a.ts', text: 'x' })).toEqual({ toggle: false, wrap: true, copy: true, preview: false, explorer: true });
    expect(toolbarShows({ path: 'a.md', text: 'x', rendered: true })).toEqual({ toggle: true, wrap: false, copy: true, preview: false, explorer: true });
    expect(toolbarShows({ path: 'a.ts', text: '' }).copy).toBe(false);
    expect(toolbarShows({ path: 'a.ts', text: 'x', truncated: true }).copy).toBe(false);
    expect(toolbarShows({ path: 'a.html', text: 'x', canPreview: true }).preview).toBe(true);
    expect(toolbarShows({ path: 'a.ts', text: 'x', canPreview: true }).preview).toBe(false);
    expect(toolbarShows({ path: 'a.ts', host: true }).explorer).toBe(false);
    expect(toolbarShows({ path: 'a.png', attachment: true }).explorer).toBe(false);
  });
});

describe('the words of a file', () => {
  it('says the limit, the read errors and a line range', () => {
    expect(FILE_TEXT_PREVIEW_MAX_BYTES).toBe(1024 * 1024);
    expect(limitNotice(2_500_000)).toBe('Preview limited to the first 1 MB of a 2,500,000 byte file.');
    expect(READ_ERRORS.binary_file).toBe('The file is binary and cannot be displayed as text.');
    expect(Object.keys(READ_ERRORS)).toHaveLength(6);
    expect(rangeLabel(12)).toBe('L12');
    expect(rangeLabel(12, 12)).toBe('L12');
    expect(rangeLabel(12, 20)).toBe('L12 to L20');
    expect(rangeLabel(20, 12)).toBe('L12 to L20');
  });
});

describe('the breadcrumbs', () => {
  it('start with the project and end with the file', () => {
    expect(crumbsFor('anima', 'src/lib/a.ts')).toEqual([
      { label: 'anima', path: '', place: 'above', folder: true },
      { label: 'src', path: 'src', place: 'above', folder: true },
      { label: 'lib', path: 'src/lib', place: 'above', folder: true },
      { label: 'a.ts', path: 'src/lib/a.ts', place: 'current', folder: false },
    ]);
    expect(crumbsFor('anima', '.')).toEqual([{ label: 'anima', path: '', place: 'current', folder: true }]);
  });

  it('keep the deeper crumbs after the person went up, and drop a trail of another branch', () => {
    const up = crumbsFor('anima', 'src', { trail: 'src/lib/a.ts' });
    expect(up.map((crumb) => crumb.place)).toEqual(['above', 'current', 'below', 'below']);
    expect(up.at(-1)?.folder).toBe(false);
    expect(crumbsFor('anima', 'docs', { trail: 'src/lib/a.ts', folder: true }).map((crumb) => crumb.label)).toEqual(['anima', 'docs']);
    expect(parentPath('src/lib/a.ts')).toBe('src/lib');
    expect(parentPath('a.ts')).toBe('');
  });
});

describe('the menu of a folder', () => {
  const ready: FolderListing = {
    state: 'ready',
    entries: [
      { name: 'b.ts', path: 'src/b.ts', folder: false },
      { name: 'lib', path: 'src/lib', folder: true },
      { name: 'a.ts', path: 'src/a.ts', folder: false },
    ],
  };
  const at = { root: 'src', folder: 'src', project: 'anima', open: 'src/a.ts' };

  it('lists the folders, then the files, and marks the open file', () => {
    const rows = folderMenu(ready, at);
    expect(labels(rows)).toEqual(['lib', 'b.ts', 'a.ts']);
    expect(rows.map((row) => (row.kind === 'choice' ? row.icon : undefined))).toEqual(['folder', 'file', 'check']);
  });

  it('has "Back to" only after a step into a folder', () => {
    expect(labels(folderMenu(ready, { ...at, root: '', folder: 'src' })).slice(0, 2)).toEqual(['Back to anima', '-']);
    expect(labels(folderMenu({ state: 'loading' }, { ...at, folder: 'src/lib/deep' }))).toEqual(['Back to lib', '-', 'Loading folder…']);
  });

  it('says each state, and only the retry rows can be chosen', () => {
    expect(folderMenu({ state: 'loading' }, at)).toEqual([{ kind: 'choice', id: 'note:Loading folder…', label: 'Loading folder…', disabled: true }]);
    expect(folderMenu({ state: 'failed' }, at)).toEqual([{ kind: 'choice', id: 'retry', label: 'Retry loading folder', icon: 'refresh-cw' }]);
    expect(labels(folderMenu({ state: 'gone' }, at))).toEqual(['This folder is no longer available.']);
    expect(labels(folderMenu({ state: 'ready', entries: [] }, at))).toEqual(['This folder is empty.']);
    expect(labels(folderMenu({ state: 'ready', entries: [], partial: true }, at))).toEqual([
      'No entries from this folder are available in the partial workspace index.',
    ]);
    expect(labels(folderMenu({ ...ready, stale: true, partial: true }, at)).slice(3)).toEqual([
      '-', 'Refresh failed. Retry', '-', 'Some workspace entries are not shown.',
    ]);
  });

  it('reads the intent of a row', () => {
    expect(folderMenuIntent('folder:src/lib')).toEqual({ kind: 'enter', path: 'src/lib' });
    expect(folderMenuIntent('file:src/a.ts')).toEqual({ kind: 'open', path: 'src/a.ts' });
    expect(folderMenuIntent('back')).toEqual({ kind: 'back' });
    expect(folderMenuIntent('retry')).toEqual({ kind: 'retry' });
    expect(folderMenuIntent('note:x')).toBeNull();
  });
});

describe('the right-click menu of a tree row', () => {
  it('always has the two chat rows, and the others when the server can', () => {
    expect(labels(fileRowMenu())).toEqual(['Copy mention', 'Add to chat']);
    const all = fileRowMenu({ canOpen: true, revealLabel: 'Reveal in Finder', editors: [{ id: 'zed', label: 'Zed' }] });
    expect(labels(all)).toEqual(['Open', 'Reveal in Finder', '-', 'Open with', 'Zed', '-', 'Copy mention', 'Add to chat']);
    expect(labels(fileRowMenu({ editors: [{ id: 'zed', label: 'Zed' }] }))).toEqual(['Open with', 'Zed', '-', 'Copy mention', 'Add to chat']);
    expect(rowMenuEditor('editor:zed')).toBe('zed');
    expect(rowMenuEditor('open')).toBeNull();
  });

  it('has the words of each failure', () => {
    expect(ROW_MENU_PROBLEMS.copy()).toEqual({ title: 'Failed to copy mention', description: 'An error occurred.' });
    expect(ROW_MENU_PROBLEMS.editor('/w/a.ts', 'Zed')).toEqual({ title: 'Could not open in Zed', description: '/w/a.ts' });
    expect(ROW_MENU_PROBLEMS.noComposer().description).toBe('Open a chat for this project and try again.');
  });
});

describe('the width of the file explorer', () => {
  it('is held from 160 to 560px and leaves 256px to the file', () => {
    expect(explorerWidth(Number.NaN, 1000)).toBe(EXPLORER_DEFAULT);
    expect(explorerWidth(100, 1000)).toBe(160);
    expect(explorerWidth(900, 1000)).toBe(560);
    expect(explorerWidth(400, 600)).toBe(344);
    expect(explorerMax(300)).toBe(160);
  });
});

describe('the zoom of an image', () => {
  it('steps by 1.25 inside 0.05 to 8', () => {
    expect(zoomIn(1)).toBe(1.25);
    expect(zoomOut(1)).toBe(0.8);
    expect(zoomIn(7)).toBe(ZOOM_MAX);
    expect(zoomOut(0.06)).toBe(ZOOM_MIN);
  });

  it('fits the image in the viewport less 32px, and never grows it', () => {
    expect(fitScale({ width: 2000, height: 1000 }, { width: 1032, height: 800 })).toBe(0.5);
    expect(fitScale({ width: 1000, height: 2000 }, { width: 1032, height: 532 })).toBe(0.25);
    expect(fitScale({ width: 100, height: 100 }, { width: 1000, height: 1000 })).toBe(1);
    expect(fitScale({ width: 100, height: 100 }, { width: 0, height: 0 })).toBe(1);
    expect(fitScale({ width: 100_000, height: 10 }, { width: 132, height: 500 })).toBe(ZOOM_MIN);
  });

  it('says the scale and the size', () => {
    expect(percent(0.512)).toBe('51%');
    expect(percent(8)).toBe('800%');
    expect(sizeLabel({ width: 1920, height: 1080 })).toBe('1,920 × 1,080');
  });
});

describe('the lines of the source view', () => {
  it('splits on each kind of line break and holds a line to the file', () => {
    expect(sourceLines('a\r\nb\nc\n')).toEqual(['a', 'b', 'c', '']);
    expect(clampLine(0, 10)).toBe(1);
    expect(clampLine(99, 10)).toBe(10);
    expect(clampLine(4.7, 10)).toBe(4);
    expect(clampLine(Number.NaN, 10)).toBe(1);
    expect(centredTop({ top: 1000, height: 20 }, 420)).toBe(800);
    expect(centredTop({ top: 10, height: 20 }, 420)).toBe(0);
  });

  it('masks the front matter at the top of a Markdown file only', () => {
    expect(withoutFrontMatter('---\ntitle: A\n---\n# A\n')).toBe('# A\n');
    expect(withoutFrontMatter('# A\n---\nb\n---\n')).toBe('# A\n---\nb\n---\n');
  });
});
