import Guacamole from './vendor/guacamole.js';
import type { SessionTunnel } from './guacamole/tunnel';

export type ArgumentClient = Pick<SessionTunnel['client'], 'createArgumentValueStream'>;

const PLAIN_TEXT = 'text/plain';
const UTF_8 = new TextEncoder();

/** GW-015. `ArrayBufferWriter` sends UTF-8, as the 1.5.5 `StringWriter` mangles a character outside the BMP. */
export function sendArgument(client: ArgumentClient, name: string, value: string): void {
  const writer = new Guacamole.ArrayBufferWriter(client.createArgumentValueStream(PLAIN_TEXT, name));
  const bytes = UTF_8.encode(value);
  if (bytes.length > 0) writer.sendData(bytes);
  writer.sendEnd();
}
