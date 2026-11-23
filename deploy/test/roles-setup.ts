// Roles are cluster-wide, so each test file that needs them calls `installRoles()`, whichever file runs first.
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { finish } from "./finished-process.ts";
import { psql, scalar, serverDirectory } from "./postgres.ts";

export const ROOT = join(import.meta.dir, "..", "..");
export const ROLES_SQL = join(ROOT, "deploy", "postgres", "roles.sql");
export const MIGRATIONS = join(ROOT, "crates", "hub", "migrations");

export interface InstalledRoles {
  secretsFolder: string;
  passwordEnv: Record<string, string>;
}

export async function migrate(database = "anima"): Promise<void> {
  const files = readdirSync(MIGRATIONS).filter((name) => name.endsWith(".sql")).sort();
  for (const file of files) {
    const applied = await psql(["-f", join(MIGRATIONS, file)], { user: "anima_owner", database });
    if (applied.exitCode !== 0) throw new Error(`migration ${file} failed: ${applied.stderr}`);
  }
}

export async function installRoles(): Promise<InstalledRoles> {
  const secretsFolder = join(serverDirectory(), "secrets");
  const made = await finish(["/bin/sh", join(ROOT, "deploy", "bin", "make-secrets"), secretsFolder], {
    env: { ...process.env, ANIMA_SECRETS_CHOWN: "0" },
  });
  if (made.exitCode !== 0) throw new Error(`make-secrets failed: ${made.stderr}`);
  const passwordEnv = {
    ANIMA_OWNER_PASSWORD_FILE: join(secretsFolder, "owner_password"),
    ANIMA_HUB_PASSWORD_FILE: join(secretsFolder, "hub_password"),
  };
  if ((await scalar("select count(*) from pg_roles where rolname = 'anima_owner'")) === "0") {
    const result = await psql(["-f", ROLES_SQL], { env: passwordEnv });
    if (result.exitCode !== 0) throw new Error(`roles.sql failed: ${result.stderr}`);
    await migrate();
  }
  return { secretsFolder, passwordEnv };
}
