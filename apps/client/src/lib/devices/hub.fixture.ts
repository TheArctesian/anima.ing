import type { TypedDocumentNode } from '@graphql-typed-document-node/core';
import type { DocumentNode, OperationDefinitionNode } from 'graphql';
import type { Hub, HubError, HubResult, Sink, Variables } from '$lib/hub/hub';

export interface Recorded {
  readonly operation: string;
  readonly variables: Variables;
}

/** A fake `Hub` for domain client tests (D-13): an operation without an answer fails as `unreachable`. */
export function fakeHub(answers: Readonly<Record<string, unknown>> = {}) {
  const requests: Recorded[] = [];
  const subscriptions: Recorded[] = [];
  const sinks = new Map<string, Set<Sink<never>>>();
  let closed = false;

  const hub: Hub = {
    address: { origin: 'https://hub.test', http: 'https://hub.test/graphql', socket: 'wss://hub.test/graphql' },
    request: <R, V extends Variables>(document: TypedDocumentNode<R, V>, variables: V) => {
      const operation = operationName(document);
      requests.push({ operation, variables });
      return Promise.resolve(answerFor<R>(answers[operation]));
    },
    subscribe: <R, V extends Variables>(document: TypedDocumentNode<R, V>, variables: V, sink: Sink<R>) => {
      const operation = operationName(document);
      subscriptions.push({ operation, variables });
      if (closed) {
        sink.complete();
        return () => undefined;
      }
      const group = sinks.get(operation) ?? new Set<Sink<never>>();
      sinks.set(operation, group);
      const listener = sink as Sink<never>;
      group.add(listener);
      return () => {
        group.delete(listener);
      };
    },
    status: { current: { connection: 'connected', lastContactAt: new Date(0) } },
    close: () => {
      closed = true;
      for (const group of sinks.values()) {
        for (const sink of group) sink.complete();
        group.clear();
      }
      return Promise.resolve();
    },
  };

  return {
    hub,
    requests,
    subscriptions,
    push: (operation: string, value: unknown): void => {
      for (const sink of sinks.get(operation) ?? []) sink.next(value as never);
    },
    fail: (operation: string, error: HubError): void => {
      for (const sink of sinks.get(operation) ?? []) sink.error(error);
    },
    listeners: (operation: string): number => sinks.get(operation)?.size ?? 0,
  };
}

const NO_ANSWER: HubError = { code: 'unreachable', message: 'The hub did not answer. Check the VPN client.' };

function answerFor<R>(answer: unknown): HubResult<R> {
  if (answer === undefined) return { ok: false, error: NO_ANSWER };
  if (isHubError(answer)) return { ok: false, error: answer };
  return { ok: true, value: answer as R };
}

function isHubError(answer: unknown): answer is HubError {
  return typeof answer === 'object' && answer !== null && 'code' in answer && 'message' in answer;
}

function operationName(document: DocumentNode): string {
  const operation = document.definitions.find(
    (definition): definition is OperationDefinitionNode => definition.kind === 'OperationDefinition',
  );
  return operation?.name?.value ?? '';
}
