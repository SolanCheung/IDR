#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readdir, readFile, stat } from "node:fs/promises";
import { join, relative, resolve } from "node:path";

const root = resolve(process.argv[2] ?? "");
if (!process.argv[2]) {
  process.stderr.write("usage: scan_package_secrets.mjs <package-root>\n");
  process.exit(2);
}

const placeholder =
  /(?:example|sample|fake|test|dummy|placeholder|redacted|replace|changeme|your[-_]|paste|insert|not-a-real)/i;
const strongPatterns = [
  ["openai_like", /\bsk-[A-Za-z0-9_-]{20,}\b/g],
  ["private_key", /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/g],
  ["aws_access_key", /\bAKIA[0-9A-Z]{16}\b/g],
  ["google_api_key", /\bAIza[0-9A-Za-z_-]{30,}\b/g],
  ["github_token", /\b(?:ghp|github_pat)_[0-9A-Za-z_]{20,}\b/g],
  ["slack_token", /\bxox[baprs]-[0-9A-Za-z-]{20,}\b/g],
];
const assignmentPatterns = [
  [
    "json_secret_assignment",
    /"(?:apiKey|api_key|accessToken|access_token|token|secret|password)"\s*:\s*"([^"\r\n]{12,})"/gi,
  ],
  [
    "env_secret_assignment",
    /(?:^|\n)[A-Z0-9_]*(?:API_KEY|ACCESS_TOKEN|TOKEN|SECRET|PASSWORD)[ \t]*=[ \t]*([^\s#\r\n]{12,})/g,
  ],
];

const findings = [];
const skippedDirectories = new Set([
  ".git",
  "target",
  "node_modules",
  "__pycache__",
  ".next",
  ".vinext",
  ".wrangler",
  "dist",
  "coverage",
]);

async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (skippedDirectories.has(entry.name)) continue;
      await walk(path);
    } else if (entry.isFile()) {
      await scan(path);
    }
  }
}

function addFinding(path, pattern, value) {
  if (placeholder.test(value)) return;
  findings.push({
    file: relative(root, path),
    pattern,
    length: value.length,
    sha256: createHash("sha256").update(value).digest("hex"),
  });
}

async function scan(path) {
  const metadata = await stat(path);
  if (metadata.size > 5 * 1024 * 1024) return;
  const bytes = await readFile(path);
  if (bytes.includes(0)) return;
  const text = bytes.toString("utf8");
  for (const [name, pattern] of strongPatterns) {
    pattern.lastIndex = 0;
    for (const match of text.matchAll(pattern)) {
      addFinding(path, name, match[0]);
    }
  }
  for (const [name, pattern] of assignmentPatterns) {
    pattern.lastIndex = 0;
    for (const match of text.matchAll(pattern)) {
      addFinding(path, name, match[1]);
    }
  }
}

await walk(root);
if (findings.length > 0) {
  process.stderr.write(`${JSON.stringify({ count: findings.length, findings }, null, 2)}\n`);
  process.exit(1);
}
process.stdout.write('{"count":0,"findings":[]}\n');
