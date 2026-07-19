#!/usr/bin/env node
/**
 * Bogahost native — üretilen platform/build çıktılarını temizler.
 *
 * SİLİNİR (CI'da yeniden üretilir):
 *   capacitor/<key>/android      (cap add android çıktısı)
 *   capacitor/<key>/ios          (cap add ios çıktısı)
 *   capacitor/<key>/resources    (gen-icons çıktısı)
 *   tauri/<key>/src-tauri/target, tauri/<key>/target
 *   dist/, out/
 *
 * SİLİNMEZ (el ile yazılan kaynak):
 *   capacitor/<key>/www          (bootstrap index.html — kaynaktır)
 *   *-overrides/, capacitor.config.ts, package.json
 *
 * Kullanım: node scripts/clean.mjs [--app <key|all>] [--deep]
 *   --deep  node_modules'ü de siler.
 * Node 20+.
 */
import { rmSync, existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');

function parseArgs(argv) {
  const out = { app: 'all', deep: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--deep') out.deep = true;
    else if (argv[i] === '--app') out.app = argv[++i];
    else if (argv[i].startsWith('--app=')) out.app = argv[i].split('=')[1];
  }
  return out;
}

function loadApps() {
  const cfg = JSON.parse(readFileSync(join(ROOT, 'apps.config.json'), 'utf8'));
  return cfg.apps || [];
}

function del(relPath) {
  const p = join(ROOT, relPath);
  if (existsSync(p)) {
    rmSync(p, { recursive: true, force: true });
    console.log(`  ✓ silindi: ${relPath}`);
    return 1;
  }
  return 0;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const apps = loadApps();
  const targets = args.app === 'all' ? apps : apps.filter((a) => a.key === args.app);

  if (targets.length === 0) {
    console.error(`✗ Bilinmeyen uygulama: "${args.app}". Geçerli: ${apps.map((a) => a.key).join(', ')}`);
    process.exit(2);
  }

  console.log('Bogahost native — temizlik\n');
  let n = 0;

  for (const app of targets) {
    console.log(`[${app.key}]`);
    n += del(`capacitor/${app.key}/android`);
    n += del(`capacitor/${app.key}/ios`);
    n += del(`capacitor/${app.key}/resources`);
    n += del(`tauri/${app.key}/src-tauri/target`);
    n += del(`tauri/${app.key}/target`);
    if (args.deep) n += del(`capacitor/${app.key}/node_modules`);
  }

  if (args.app === 'all') {
    console.log('[kök]');
    n += del('dist');
    n += del('out');
    if (args.deep) n += del('node_modules');
  }

  console.log(`\n${n} öğe temizlendi.`);
  if (n === 0) console.log('(Zaten temizdi.)');
}

main();
