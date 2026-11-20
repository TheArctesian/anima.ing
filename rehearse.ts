import { finishMerge } from "./quality/src/merge/finish.ts";
import { stack } from "./quality/src/merge/stack.ts";
const root = process.cwd();
const names = process.argv.slice(2);
const tips = names.map((branch) => ({ branch, sha: Bun.spawnSync(["git", "rev-parse", `origin/${branch}`], { cwd: root }).stdout.toString().trim() }));
const made = stack(root, tips, (tip) => { const done = finishMerge(root, tip.branch, "dev"); return "failure" in done ? done.failure : undefined; });
console.log("merged:", made.merged.map((t) => t.branch).join(", "));
for (const each of made.leftOut) console.log("left out:", each.branch, "-", each.reason.split("\n")[0]);
