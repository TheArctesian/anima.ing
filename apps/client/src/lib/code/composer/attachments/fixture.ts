import type { Attachment } from './attach';

/* A picture drawn here for the specimen page and the tests: flat shapes in named colours (they are pixels of a picture, not colours of the interface). */
const PICTURE =
  'data:image/svg+xml,' +
  encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" fill="steelblue"/><circle cx="44" cy="22" r="10" fill="khaki"/><path d="M0 64 24 34l16 18 8-8 16 20z" fill="darkslategray"/></svg>');

/** Made-up attachments, one of each sort and state the composer draws. */
export const ATTACHMENTS: readonly Attachment[] = [
  { id: 'a-image', name: 'hill-at-noon.png', kind: 'image', size: 284_512, mime: 'image/png', preview: PICTURE, state: { state: 'ready' } },
  { id: 'a-reading', name: 'screen-recording-frame.jpg', kind: 'image', size: 6_815_744, mime: 'image/jpeg', preview: null, state: { state: 'reading' } },
  { id: 'a-failed', name: 'scan.heic', kind: 'image', size: 2_097_152, mime: 'image/heic', preview: null, state: { state: 'failed', words: "'scan.heic' could not be read as an image." } },
  { id: 'a-video', name: 'walkthrough.mp4', kind: 'video', size: 18_350_080, mime: 'video/mp4', preview: null, state: { state: 'ready' } },
  { id: 'a-text', name: 'pasted-text.txt', kind: 'text', size: 48_211, mime: 'text/plain;charset=utf-8', preview: null, state: { state: 'ready' } },
  { id: 'a-pdf', name: 'network-survey-2026.pdf', kind: 'file', size: 1_572_864, mime: 'application/pdf', preview: null, state: { state: 'ready' } },
  { id: 'a-code', name: 'a-rather-long-file-name-that-has-to-be-cut-short-in-the-list.ts', kind: 'file', size: 900, mime: 'video/mp2t', preview: null, state: { state: 'reading' } },
  { id: 'a-lost', name: 'export.csv', kind: 'text', size: 0, mime: 'text/csv', preview: null, state: { state: 'failed', words: "'export.csv' is empty or could not be read." } },
];
