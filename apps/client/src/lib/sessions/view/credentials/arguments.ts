import { sendArgument, type ArgumentClient } from '../../tunnel/argv';
import type { CredentialField, CredentialName } from './fields';

export type Answers = Readonly<Partial<Record<CredentialName, string>>>;

/** An empty field is sent empty so the gateway is not left waiting; names outside `fields` are never sent (GW-015). */
export function sendArguments(client: ArgumentClient, fields: readonly CredentialField[], answers: Answers): void {
  for (const { name } of fields) sendArgument(client, name, answers[name] ?? '');
}
