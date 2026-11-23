// Windows PowerShell 5 drops empty environment values; the empty RUSTC_WRAPPER keeps Cargo off the Unix cache script.
const command = process.argv.slice(2);
if (command.length === 0) throw new Error("A native build command is required");
const child = Bun.spawn(command, {
  env: { ...process.env, RUSTC_WRAPPER: "" },
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit",
});
process.exit(await child.exited);
