#!/usr/bin/env node
/**
 * Bogahost native — çok platformlu derleme dağıtıcısı.
 *
 * Kullanım:
 *   node scripts/build.mjs --platform <android|ios|windows|mac|all> --app <key|all>
 *   node scripts/build.mjs --pwa
 *
 * Ne yapar:
 *   - android/ios  → capacitor/<key> içinde `npm run build:<platform>` çağırır.
 *   - windows/mac  → tauri/<key> içinde `npm run tauri:build` çağırır (Tauri projesi
 *                    başka bir ajan tarafından yazılır; burada yalnızca o yola dispatch edilir).
 *   - --pwa        → PWA zaten canlı sitede yayında; sadece bilgi/doküman basar.
 *
 * Toolchain (gradle/xcode/cargo) yoksa NET hata mesajı + doküman linki basılır;
 * süreç çökmez, ilgili adım atlanır ve çıkış kodu 1 olur.
 *
 * Node 20+, sadece stdlib.
 */
import { spawnSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const DOCS = 'Ayrıntı: capacitor/README.md';
const IS_WIN = process.platform === 'win32';

const VALID_PLATFORMS = ['android', 'ios', 'windows', 'mac'];

function parseArgs(argv) {
  const out = { platform: null, app: 'all', pwa: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--pwa') out.pwa = true;
    else if (a === '--platform') out.platform = argv[++i];
    else if (a === '--app') out.app = argv[++i];
    else if (a.startsWith('--platform=')) out.platform = a.split('=')[1];
    else if (a.startsWith('--app=')) out.app = a.split('=')[1];
  }
  return out;
}

function loadApps() {
  const cfgPath = join(ROOT, 'apps.config.json');
  const cfg = JSON.parse(readFileSync(cfgPath, 'utf8'));
  return cfg.apps || [];
}

function selectApps(apps, key) {
  if (!key || key === 'all') return apps;
  const found = apps.filter((a) => a.key === key);
  if (found.length === 0) {
    console.error(`✗ Bilinmeyen uygulama: "${key}". Geçerli: ${apps.map((a) => a.key).join(', ')}`);
    process.exit(2);
  }
  return found;
}

/** Bir komutu çalıştırır; başarısızlıkta çökmeden false döner. */
function run(cmd, args, cwd) {
  console.log(`\n$ ${cmd} ${args.join(' ')}  (cwd: ${cwd})`);
  const res = spawnSync(cmd, args, { cwd, stdio: 'inherit', shell: IS_WIN });
  if (res.error) {
    if (res.error.code === 'ENOENT') {
      console.error(`✗ "${cmd}" bulunamadı — bu platformun toolchain'i kurulu değil. ${DOCS}`);
    } else {
      console.error(`✗ Komut hatası: ${res.error.message}. ${DOCS}`);
    }
    return false;
  }
  if (res.status !== 0) {
    console.error(`✗ "${cmd}" çıkış kodu ${res.status}. Toolchain/imzalama eksik olabilir. ${DOCS}`);
    return false;
  }
  return true;
}

function buildCapacitor(app, platform) {
  const appDir = join(ROOT, 'capacitor', app.key);
  if (!existsSync(join(appDir, 'package.json'))) {
    console.error(`✗ ${appDir} bulunamadı — Capacitor kabuğu eksik.`);
    return false;
  }
  console.log(`\n▶ [${app.key}] ${platform} (Capacitor WebView → ${app.url})`);
  return run('npm', ['run', `build:${platform}`], appDir);
}

function buildTauri(app, platform) {
  // windows|mac → Tauri. Proje yolu konvansiyonu: tauri/<key>/
  const appDir = join(ROOT, 'tauri', app.key);
  if (!existsSync(join(appDir, 'package.json'))) {
    console.error(
      `✗ tauri/${app.key} bulunamadı. Masaüstü (${platform}) kabuğu Tauri ile ayrı ajan tarafından yazılır. ${DOCS}`
    );
    return false;
  }
  // Tauri projeleri platforma özel script sunar; yoksa genel tauri:build'e düş.
  const script = platform === 'windows' ? 'tauri:build:win' : 'tauri:build:mac';
  const scripts = readTauriScripts(appDir);
  const target = scripts.includes(script) ? script : 'tauri:build';
  console.log(`\n▶ [${app.key}] ${platform} (Tauri → ${app.url})`);
  return run('npm', ['run', target], appDir);
}

function readTauriScripts(appDir) {
  try {
    const pkg = JSON.parse(readFileSync(join(appDir, 'package.json'), 'utf8'));
    return Object.keys(pkg.scripts || {});
  } catch {
    return [];
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const apps = loadApps();

  if (args.pwa) {
    console.log('ℹ PWA: 4 uygulama zaten canlı sitede PWA olarak yayında (manifest + service worker sunucuda).');
    console.log('  Native kabuk bu PWA URL\'lerini WebView içinde yükler; ayrı bir PWA derlemesi gerekmez.');
    console.log(`  ${DOCS}`);
    return;
  }

  const platformOk = args.platform === 'all' || VALID_PLATFORMS.includes(args.platform);
  if (!platformOk) {
    console.error('Kullanım: node scripts/build.mjs --platform <android|ios|windows|mac|all> [--app <key|all>] | --pwa');
    process.exit(2);
  }

  const platforms = args.platform === 'all' ? VALID_PLATFORMS : [args.platform];
  const targets = selectApps(apps, args.app);

  let failures = 0;
  let total = 0;

  for (const platform of platforms) {
    for (const app of targets) {
      total++;
      let ok;
      if (platform === 'android' || platform === 'ios') ok = buildCapacitor(app, platform);
      else ok = buildTauri(app, platform); // windows | mac
      if (!ok) failures++;
    }
  }

  console.log(`\n─── Özet: ${total - failures}/${total} hedef başarılı ───`);
  if (failures > 0) {
    console.error(`${failures} hedef atlandı/başarısız (büyük olasılıkla eksik toolchain). ${DOCS}`);
    process.exitCode = 1;
  }
}

main();
