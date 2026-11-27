import type { Gate } from "./gates.ts";

/** Source checks cannot substitute for the Linux controller and installed-image tests. */
export const OS_SOURCE: Gate = {
  name: "os-source",
  group: "quality",
  argv: ["uv", "run", "--no-project", "--with", "boto3==1.42.6", "python", "os/check.py"],
  requires: [{ executable: "uv", install: "brew install uv", folder: "/opt/homebrew/bin" }],
};
