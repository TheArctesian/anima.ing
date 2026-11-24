import type { ArgumentClient } from '../../tunnel/argv';

export interface ArgumentStream {
  readonly mimetype: string;
  readonly name: string;
  readonly blobs: string[];
  ends: number;
}

function decoded(blobs: readonly string[]): string {
  return Buffer.concat(blobs.map((blob) => Buffer.from(blob, 'base64'))).toString('utf8');
}

export function fakeArgumentClient() {
  const streams: ArgumentStream[] = [];
  const client: ArgumentClient = {
    createArgumentValueStream: (mimetype, name) => {
      const stream: ArgumentStream = { mimetype, name, blobs: [], ends: 0 };
      streams.push(stream);
      const output = {
        index: streams.length,
        onack: null,
        sendBlob: (blob: string) => stream.blobs.push(blob),
        sendEnd: () => {
          stream.ends += 1;
        },
      };
      return output as unknown as ReturnType<ArgumentClient['createArgumentValueStream']>;
    },
  };
  const sent = () => streams.map(({ mimetype, name, blobs, ends }) => ({ mimetype, name, text: decoded(blobs), ends }));
  return { client, sent };
}
