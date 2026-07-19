#!/usr/bin/env node
/**
 * Bogahost native — geliştirme yardımcısı.
 *
 * Kabuklar CANLI URL'yi yükler; ayrı bir dev sunucusu YOKTUR. Bu yardımcı seçilen
 * uygulamanın canlı URL'sini varsayılan tarayıcıda açar ve emülatör/cihaz için
 * yönergeleri basar.
 *
 * Kullanım:
 *   node scripts/dev.mjs --app <key|all> [--open]
 *
 * Node 20+, sadece stdlib.
 */
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');

function parseArgs(argv) {
  const out = { app: 'all', open: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--open') out.open = true;
    else if (a === '--app') out.app = argv[++i];
    else if (a.startsWith('--app=')) out.app = a.split('=')[1];
  }
  return out;
}

function loadApps() {
  const cfg = JSON.parse(readFileSync(join(ROOT, 'apps.config.json'), 'utf8'));
  return cfg.apps || [];
}

function openInBrowser(url) {
  let cmd, args;
  if (process.platform === 'darwin') { cmd = 'open'; args = [url]; }
  else if (process.platform === 'win32') { cmd = 'cmd'; args = ['/c', 'start', '', url]; }
  else { cmd = 'xdg-open'; args = [url]; }
  try {
    const child = spawn(cmd, args, { stdio: 'ignore', detached: true });
    child.on('error', () => console.log(`  (tarayıcı otomatik açılamadı — URL'yi elle aç: ${url})`));
    child.unref();
  } catch {
    console.log(`  (tarayıcı otomatik açılamadı — URL'yi elle aç: ${url})`);
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const apps = loadApps();
  const targets = args.app === 'all' ? apps : apps.filter((a) => a.key === args.app);

  if (targets.length === 0) {
    console.error(`✗ Bilinmeyen uygulama: "${args.app}". Geçerli: ${apps.map((a) => a.key).join(', ')}`);
    process.exit(2);
  }

  console.log('Bogahost native — kabuklar canlı URL yükler (ayrı dev sunucusu yok).\n');
  for (const app of targets) {
    console.log(`• ${app.name} [${app.key}]`);
    console.log(`    URL     : ${app.url}`);
    console.log(`    Android : npm --prefix capacitor/${app.key} run open:android  → Android Studio → emülatör/cihaz`);
    console.log(`    iOS     : npm --prefix capacitor/${app.key} run open:ios      → Xcode → simülatör/cihaz`);
    if (args.open) openInBrowser(app.url);
    console.log('');
  }

  if (!args.open) {
    console.log('İpucu: canlı URL\'yi tarayıcıda açmak için --open ekleyin.');
  }
}

main();
