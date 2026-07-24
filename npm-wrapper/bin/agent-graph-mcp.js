#!/usr/bin/env node
/**
 * npm wrapper for agent-graph-mcp Rust binary.
 *
 * Tries (in order):
 * 1. A pre-built binary downloaded from GitHub releases
 * 2. A cargo-installed binary from crates.io
 * 3. Falls back to telling the user to install Rust
 */

const { execSync, spawn } = require("child_process");
const os = require("os");
const path = require("path");
const fs = require("fs");

const PLATFORM = os.platform();
const ARCH = os.arch();
const VERSION = "1.0.2";

const binaryName = "agent-graph-mcp";
const binDir = path.join(__dirname, "..", ".bin-cache");
const binPath = path.join(binDir, binaryName);

function getDownloadUrl() {
  const platformMap = {
    "linux-x64": `https://github.com/RecursiveIntell/agent-graph-mcp/releases/download/v${VERSION}/agent-graph-mcp-linux-x64`,
    "darwin-x64": `https://github.com/RecursiveIntell/agent-graph-mcp/releases/download/v${VERSION}/agent-graph-mcp-darwin-x64`,
    "darwin-arm64": `https://github.com/RecursiveIntell/agent-graph-mcp/releases/download/v${VERSION}/agent-graph-mcp-darwin-arm64`,
  };
  const key = `${PLATFORM}-${ARCH}`;
  return platformMap[key] || null;
}

function ensureBinary() {
  if (fs.existsSync(binPath)) {
    try { fs.chmodSync(binPath, 0o755); } catch {}
    return binPath;
  }

  const url = getDownloadUrl();
  if (url) {
    if (!fs.existsSync(binDir)) fs.mkdirSync(binDir, { recursive: true });
    try {
      execSync(`curl -sL -o "${binPath}" "${url}"`, { stdio: "pipe" });
      fs.chmodSync(binPath, 0o755);
      return binPath;
    } catch {}
  }

  try {
    execSync(`which ${binaryName}`, { stdio: "pipe" });
    return binaryName;
  } catch {}

  try {
    execSync("cargo install agent-graph-mcp --locked", { stdio: "inherit" });
    return binaryName;
  } catch {
    process.stderr.write(
      `No pre-built binary for ${PLATFORM}-${ARCH} and cargo is not available.\n` +
      `Install Rust from https://rustup.rs and run: cargo install agent-graph-mcp --locked\n`
    );
    return null;
  }
}

const binary = ensureBinary();
if (!binary) {
  process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
});
child.on("exit", (code) => process.exit(code || 0));
