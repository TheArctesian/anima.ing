// What the synthetic hub saw. Only facts about headers are kept, never a header's value (LOG-007).

export type Channel = 'graphql' | 'graphql-socket' | 'terminal' | 'tunnel';

export interface RecordedRequest {
  readonly channel: Channel;
  /** The GraphQL operation name, or its first root field for an unnamed one; `null` for a tunnel. */
  readonly operation: string | null;
  readonly variables: unknown;
  readonly hadAuthorization: boolean;
  readonly hadCookie: boolean;
  readonly origin: string | null;
  readonly signedIn: boolean;
}

export interface Recorder {
  readonly requests: readonly RecordedRequest[];
  /** Every request to another origin was aborted; this counts them. Specs assert 0. */
  readonly foreignRequests: number;
  /** The origins (never paths or queries) that were refused. */
  readonly foreignOrigins: readonly string[];
  where(match: { channel?: Channel; operation?: string }): RecordedRequest[];
}

export interface Recording extends Recorder {
  record(request: RecordedRequest): void;
  refuseForeign(url: string): void;
}

export function newRecording(): Recording {
  const requests: RecordedRequest[] = [];
  const foreignOrigins: string[] = [];
  return {
    requests,
    get foreignRequests() {
      return foreignOrigins.length;
    },
    foreignOrigins,
    where: ({ channel, operation }) =>
      requests.filter((found) => (channel === undefined || found.channel === channel) && (operation === undefined || found.operation === operation)),
    record: (request) => requests.push(request),
    refuseForeign: (url) => foreignOrigins.push(URL.canParse(url) ? new URL(url).origin : 'unparsable'),
  };
}
