// A workspace build unites the features of its crates, so a crate can build there and fail alone, as release builds do.

export type Run = (argv: string[]) => number;

export function checkAlone(crates: string[], run: Run, report: (line: string) => void): number {
  const failed: string[] = [];
  for (const [index, crate] of crates.entries()) {
    const argv = ["cargo", "check", "-p", crate, "--locked"];
    report(`[${index + 1}/${crates.length}] ${argv.join(" ")}`);
    if (run(argv) !== 0) failed.push(crate);
  }
  if (failed.length === 0) {
    report(`alone: each of ${crates.length} crates builds alone`);
    return 0;
  }
  report(`alone: ${failed.length} crate(s) do not build alone: ${failed.join(", ")}. Each crate must name in its own Cargo.toml each feature that it uses`);
  return 1;
}
