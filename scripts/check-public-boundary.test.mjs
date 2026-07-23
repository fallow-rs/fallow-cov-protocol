import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { checkPublicBoundary } from "./check-public-boundary.mjs";

const fixture = async (source) => {
  const root = await mkdtemp(join(tmpdir(), "fallow-protocol-boundary-"));
  await mkdir(join(root, "src"), { recursive: true });
  await writeFile(join(root, "src", "lib.rs"), source);
  await writeFile(join(root, "README.md"), "# Public protocol\n");
  await writeFile(join(root, "CONTRIBUTING.md"), "# Contributing\n");
  await writeFile(join(root, "CHANGELOG.md"), "# Changelog\n");
  await writeFile(join(root, "CLAUDE.md"), "# Agent router\n");
  return root;
};

test("accepts portable public protocol documentation", async () => {
  const root = await fixture("//! Public wire contract.\n");
  assert.deepEqual(await checkPublicBoundary({ root }), []);
});

test("rejects private and machine-local references", async () => {
  const root = await fixture(
    "//! See .internal/private.md, `internal/runbook.md`, and git://github.com/fallow-rs/fallow-cloud.git in /Users/example/project.\n",
  );
  const errors = await checkPublicBoundary({ root });
  assert.ok(errors.some((error) => error.includes("private internal path")));
  assert.ok(errors.some((error) => error.includes("plain private root")));
  assert.ok(errors.some((error) => error.includes("private repository URL")));
  assert.ok(errors.some((error) => error.includes("machine-local path")));
});
