import { test } from "node:test";
import assert from "node:assert/strict";
import { writeFile, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { sha256File } from "./sovereign-models.mjs";

test("sha256File matches the known digest of 'hello world'", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "sov-"));
  const f = path.join(dir, "x.txt");
  await writeFile(f, "hello world");
  // printf 'hello world' | sha256sum
  assert.equal(
    await sha256File(f),
    "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
  );
});
