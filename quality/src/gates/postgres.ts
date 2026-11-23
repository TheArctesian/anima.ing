import { join } from "node:path";

/** PostgreSQL major version the hub targets (spec DB-001). */
export const POSTGRES_MAJOR = 17;

export function binaryDirs(pathEnv: string, major: number = POSTGRES_MAJOR): string[] {
  const fromPath = pathEnv.split(":").filter((dir) => dir !== "");
  return [
    `/opt/homebrew/opt/postgresql@${major}/bin`,
    `/usr/local/opt/postgresql@${major}/bin`,
    `/usr/lib/postgresql/${major}/bin`,
    ...fromPath,
  ];
}

export function findTools(dirs: string[], tools: string[], exists: (path: string) => boolean): string | undefined {
  return dirs.find((dir) => tools.every((tool) => exists(join(dir, tool))));
}

export function socketDatabaseUrl(socketDir: string, port: number, user: string): string {
  return `postgres://${encodeURIComponent(user)}@localhost:${port}/postgres?host=${encodeURIComponent(socketDir)}`;
}

/** fsync is off: the data is thrown away. */
export function serverOptions(socketDir: string, port: number): string {
  return `-c listen_addresses='' -c unix_socket_directories='${socketDir}' -c port=${port} -c fsync=off -c max_connections=200`;
}
