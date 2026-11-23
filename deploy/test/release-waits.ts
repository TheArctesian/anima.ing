// GitHub skips a job after a skipped one, and the gates skip content that passed on `dev`, so a job names each result it waits for.

export const LEFT_OUT: Readonly<Record<string, string>> = {
  windows: "(needs.windows.result == 'success' || needs.commit.outputs.release_windows != 'true')",
};

export function eachPassed(needs: readonly string[]): string {
  return ["!cancelled()", "!failure()", ...needs.map((name) => LEFT_OUT[name] ?? `needs.${name}.result == 'success'`)].join(" && ");
}

export function ownCondition(condition: string | undefined, needs: readonly string[]): string {
  const start = `\${{ ${eachPassed(needs)}`;
  if (condition === undefined || !condition.startsWith(start) || !condition.endsWith(" }}")) {
    throw new Error(`the condition does not name the result of each job that it waits for: ${condition ?? "none"}`);
  }
  const rest = condition.slice(start.length, -" }}".length);
  if (rest === "") return "";
  if (!rest.startsWith(" && ")) throw new Error(`the condition is not joined with &&: ${condition}`);
  return rest.slice(" && ".length);
}
