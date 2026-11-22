// Rules for .github/workflows/hub-image.yml, checked on the parsed file: the build of the hub image
// holds no secret, can write nothing, publishes nothing, and only a branch image/* starts it.
// Nothing here runs the workflow.
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..");
const RAW = readFileSync(join(ROOT, ".github", "workflows", "hub-image.yml"), "utf8");

interface Step {
  uses?: string;
  run?: string;
  with?: Record<string, string | boolean | number>;
}
interface Job {
  "runs-on": string;
  environment?: string;
  permissions?: Record<string, string>;
  steps: Step[];
}
interface Workflow {
  on: Record<string, unknown>;
  permissions: Record<string, string>;
  jobs: Record<string, Job>;
}

const workflow = Bun.YAML.parse(RAW) as Workflow;
const jobs = Object.entries(workflow.jobs);
const steps = jobs.flatMap(([, job]) => job.steps);

describe("the hub image workflow (hub-image.yml)", () => {
  test("only a push of a branch image/* starts it", () => {
    expect(workflow.on).toEqual({ push: { branches: ["image/**"] } });
  });

  test("it holds no secret and can only read the repository", () => {
    expect(RAW).not.toContain("secrets.");
    expect(workflow.permissions).toEqual({});
    for (const [name, job] of jobs) {
      expect(job.permissions, name).toEqual({ contents: "read" });
      expect(job.environment, name).toBeUndefined();
      expect(job["runs-on"], name).toBe("ubuntu-24.04");
    }
    for (const step of steps.filter((each) => each.uses?.startsWith("actions/checkout@"))) {
      expect(step.with?.["persist-credentials"]).toBe(false);
    }
  });

  test("it publishes nothing: no registry, no push, a short-lived artifact", () => {
    const scripts = steps.map((step) => step.run ?? "").join("\n");
    expect(scripts).not.toMatch(/docker (login|push)|--push\b|ghcr\.io/);
    const upload = steps.find((step) => step.uses?.startsWith("actions/upload-artifact@"));
    expect(upload?.with?.["retention-days"]).toBe(3);
  });

  test("each action is pinned to a commit, and no run script holds an expression", () => {
    for (const step of steps) {
      if (step.uses !== undefined) expect(step.uses).toMatch(/^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/);
      expect(step.run ?? "").not.toContain("${{");
    }
  });
});
