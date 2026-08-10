import test from "node:test";
import assert from "node:assert/strict";
import { resolveWithHostModel } from "../src/host-model-provider.ts";

test("uses host model when no intent candidate exists", async () => {
  let called = false;
  const result = await resolveWithHostModel({
    async infer() {
      called = true;
      return { content: "resolved" };
    },
  }, { request: { task: "test" } });

  assert.equal(called, true);
  assert.equal(result?.content, "resolved");
});

test("does not duplicate host reasoning", async () => {
  let called = false;
  const result = await resolveWithHostModel({
    async infer() {
      called = true;
      return { content: "unexpected" };
    },
  }, { request: { task: "test" }, intentCandidate: { intent: "known" } });

  assert.equal(called, false);
  assert.equal(result, undefined);
});
