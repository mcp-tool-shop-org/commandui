// Release gate: the release tag must equal every version the installers are built from.
// Usage: node packaging/check-release-version.mjs <tag>   (e.g. v1.0.2)
// Exit 0 on match, 1 on any mismatch or unreadable source (fails closed).
// A pre-release tag such as v1.1.0-rc1 never matches the plain X.Y.Z sources and
// is rejected on purpose: bump the sources to that exact string first.
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

function cargoPackageVersion(text) {
  const section = text.split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0] ?? '';
  return section.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
}

const tag = (process.argv[2] ?? '').replace(/^v/, '');
try {
  const versions = {
    'tauri.conf.json': JSON.parse(read('apps/desktop/src-tauri/tauri.conf.json')).version,
    'package.json': JSON.parse(read('apps/desktop/package.json')).version,
    'Cargo.toml': cargoPackageVersion(read('apps/desktop/src-tauri/Cargo.toml')),
    AppxManifest: read('packaging/msix/AppxManifest.xml')
      .match(/<Identity\b[^>]*\bVersion="([^"]+)"/)?.[1]
      ?.replace(/\.0$/, ''),
  };
  console.log(`tag=${tag}`, versions);
  const bad = Object.entries(versions).filter(([, v]) => !tag || v !== tag);
  if (bad.length) {
    console.error(`Release tag ${process.argv[2]} does not match: ${bad.map(([k]) => k).join(', ')}; bump the sources before tagging.`);
    process.exit(1);
  }
} catch (e) {
  console.error(`Version check failed: ${e.message}`);
  process.exit(1);
}
