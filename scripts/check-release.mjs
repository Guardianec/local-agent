import { execFileSync } from 'node:child_process';
import { readFileSync, statSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import assert from 'node:assert/strict';

// Values are never printed: a failed audit identifies only the path/category.
const privatePath = /(?:[A-Z]:[\\/]Users[\\/][^\s"'<>]+|\/(?:Users|home)\/[^/\s]+)/i;
const secret = /(?:-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{30,}|AKIA[0-9A-Z]{16}|xox[baprs]-[0-9A-Za-z-]{16,})/;
assert(privatePath.test('C:' + '\\Users\\someone\\private'));
assert(!privatePath.test('src/features/tasks'));
assert(secret.test('ghp_' + 'a'.repeat(30)));
assert(!secret.test('example-placeholder'));
const fail = (path, reason) => { throw Error(`${path}: ${reason}`); };
const binaryIndex = process.argv.indexOf('--binary');
if (binaryIndex !== -1) {
  for (const name of process.argv.slice(binaryIndex + 1)) {
    const bytes = readFileSync(name);
    for (const text of [bytes.toString('latin1'), bytes.toString('utf16le')]) {
      if (privatePath.test(text)) fail(name, 'embedded private build path');
      if (secret.test(text)) fail(name, 'credential-shaped data');
    }
  }
  console.log('PASS: public binary path/credential checks');
} else {
  const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
  const lock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
  const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
  const cargoLock = readFileSync('src-tauri/Cargo.lock', 'utf8').match(/name = "local-agent"\r?\nversion = "([^"]+)"/)?.[1];
  for (const v of [lock.version, lock.packages[''].version, tauri.version, cargo, cargoLock]) assert.equal(v, pkg.version, 'Desktop versions disagree');
  const files = [...new Set(execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).toString().split('\0').filter(Boolean))];
  for (const name of files) {
    if (!existsSync(name) || !statSync(name).isFile()) continue;
    if (/(?:^|\/)(?:\.env(?:\..*)?|id_rsa|id_ed25519|credentials|.*\.(?:exe|msi|msix|pdb|pem|pfx|p12|key|sqlite(?:-wal|-shm)?|db|log|gguf|safetensors|onnx|zip|7z|tmp|bak))$/i.test(name)) fail(name, 'private/generated file cannot be published');
    if (statSync(name).size > 2 * 1024 * 1024) fail(name, 'unexpected large source asset');
    const bytes = readFileSync(name);
    if (bytes.subarray(0, 8192).includes(0)) {
      if (!/^src-tauri\/icons\/.*\.(?:png|ico|icns)$/.test(name) && name !== 'docs/images/workspace-review.png') fail(name, 'unexpected binary in source');
      continue;
    }
    const text = bytes.toString('utf8');
    if (privatePath.test(text)) fail(name, 'private machine path');
    if (secret.test(text)) fail(name, 'credential-shaped data');
    if (name.endsWith('.md')) {
      for (const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
        const link = match[1].split('#')[0];
        if (!link || /^(?:https?:|mailto:)/.test(link)) continue;
        if (!existsSync(resolve(dirname(name), decodeURIComponent(link)))) fail(name, `missing local documentation link: ${link}`);
      }
    }
  }
  for (const name of ['LICENSE', 'README.md', 'CHANGELOG.md', 'CONTRIBUTING.md', 'AGENTS.md', 'SECURITY.md', 'docs/DEVELOPMENT_HISTORY.md']) assert(existsSync(name), `${name} missing`);
  console.log(`PASS: ${files.length} source candidates, local documentation links and desktop version ${pkg.version}`);
}
