import { describe, expect, test } from "bun:test";
import { ATTEMPTS, outcomeOf, startOf } from "../../src/merge/each.ts";

describe("merge --each: merge branches one at a time while dev moves", () => {
  test("a branch that origin lacks is absent, and one that dev holds needs no merge", () => {
    expect(startOf(false, false)).toBe("absent");
    expect(startOf(true, true)).toBe("in-dev");
    expect(startOf(true, false)).toBe("merge");
  });

  test("a failure while dev moved runs again, up to the limit", () => {
    expect(outcomeOf({ exit: 0, devBefore: "a", devAfter: "b", number: 1 })).toBe("merged");
    expect(outcomeOf({ exit: 4, devBefore: "a", devAfter: "b", number: 1 })).toBe("again");
    expect(outcomeOf({ exit: 4, devBefore: "a", devAfter: "b", number: ATTEMPTS })).toBe("stopped");
  });

  test("a failure on a dev that did not move is the answer for the branch", () => {
    expect(outcomeOf({ exit: 5, devBefore: "a", devAfter: "a", number: 1 })).toBe("stopped");
  });
});
