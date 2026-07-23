#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));
const PUBLIC_ROOTS = ["src", "README.md", "CONTRIBUTING.md", "CHANGELOG.md", "CLAUDE.md"];
const TEXT_EXTENSIONS = new Set([".md", ".rs"]);
const FORBIDDEN = [
  {
    label: "private repository URL",
    pattern:
      /(?:(?:(?:https?|git):)?\/\/(?:www\.)?github\.com\/|ssh:\/\/git@github\.com\/|git@github\.com:)fallow-rs\/fallow-cloud(?:\.git)?(?=$|[/?#:\s`),.])/iu,
  },
  { label: "private internal path", pattern: /(?:^|[^\w-])\.internal\//u },
  { label: "plain private root", pattern: /(?:^|[\s([{"'=`])internal\//u },
  { label: "private decision path", pattern: /(?:^|[^\w@-])decisions\//u },
  { label: "machine-local path", pattern: /\/Users\//u },
];

const collect = async (path) => {
  const entries = await readdir(path, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collect(child)));
    } else if (entry.isFile() && TEXT_EXTENSIONS.has(extname(entry.name))) {
      files.push(child);
    }
  }
  return files;
};

export const checkPublicBoundary = async ({ root = ROOT } = {}) => {
  const errors = [];
  for (const publicRoot of PUBLIC_ROOTS) {
    const path = join(root, publicRoot);
    const files = extname(path) === "" ? await collect(path) : [path];
    for (const file of files) {
      const content = await readFile(file, "utf8");
      for (const forbidden of FORBIDDEN) {
        if (forbidden.pattern.test(content)) {
          errors.push(`${relative(root, file)} exposes a ${forbidden.label}`);
        }
      }
    }
  }
  return errors;
};

const isMain = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  checkPublicBoundary()
    .then((errors) => {
      if (errors.length > 0) {
        for (const error of errors) {
          process.stderr.write(`public boundary: ${error}\n`);
        }
        process.exitCode = 1;
        return;
      }
      process.stdout.write("Public protocol boundary is clean.\n");
    })
    .catch((error) => {
      process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
      process.exitCode = 2;
    });
}
