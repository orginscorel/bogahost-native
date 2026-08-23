#!/usr/bin/env node
// Yalnizca CI'da calisir. Indirilen build artifact'larindan
// native.bogahost.com'a yuklenecek DIZIN AGACINI hazirlar (yukleme yapmaz —
// yuklemeyi workflow'daki lftp adimi yapar).
//
//   node scripts/ci/publish-updates.mjs \
//     --artifacts dist --out out --version 1.2.0 \
//     --platform windows|macos|android \
//     --notes "Surum notu" --base-url https://native.bogahost.com
//
// URETILEN AGAC (FTP kokune = public_html/native mirror'lanir):
//
//   downloads/<app>-<version>-<platform>.<ext>        installer'lar (+ .sig)
//   updates/<app>/<target>/<arch>/latest.json         updater BIRINCIL endpoint
//   updates/<app>/latest.json                         updater YEDEK endpoint (birlesik)
//   updates/downloads.json                            index.html icin durum dosyasi
//   latest.json                                       ESKI manifest (v1.1.0 istemcileri)
//   index.html                                        indirme sayfasi
//
// NEDEN PER-ARCH DOSYA BIRINCIL?
//   windows.yml ve macos.yml AYRI kosar. Ortak tek bir latest.json'i ikisi de
//   yazsaydi, digerinin platform kaydini silerdi. Per-arch dosyalari yalnizca
//   kendi workflow'u yazdigi icin CAKISMA OLMAZ. Birlesik dosya yalnizca
//   sunucudaki kopya BASARIYLA okunabilirse guncellenir; okunamazsa
//   (ornegin Cloudflare 403 — bkz. docs/UPDATE.md) DOKUNULMAZ.

import { existsSync, mkdirSync, copyFileSync, readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, join, basename, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const HERE = dirname(fileURLToPath(import.meta.url));

function arg(name, fallback = '') {
  const i = process.argv.indexOf(`--${name}`);
  return i !== -1 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

const ARTIFACTS = resolve(arg('artifacts', 'dist'));
const OUT = resolve(arg('out', 'out'));
const VERSION = arg('version').replace(/^v/, '');
const PLATFORM = arg('platform');
const NOTES = arg('notes', `Bogahost masaüstü sürüm ${VERSION}.`);
const BASE_URL = arg('base-url', 'https://native.bogahost.com').replace(/\/+$/, '');
const APPS = ['finans', 'dcim', 'chat', 'task', 'muh', 'kasa'];
const APP_LABELS = { finans: 'Finans', dcim: 'DCIM', chat: 'Chat', task: 'Görevler', muh: 'Muhasebe', kasa: 'Kasa' };

if (!VERSION) throw new Error('--version zorunlu');
if (!['windows', 'macos', 'android'].includes(PLATFORM)) {
  throw new Error('--platform windows|macos|android olmali');
}

const PUB_DATE = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
const warnings = [];

function warn(msg) {
  warnings.push(msg);
  console.log(`::warning title=Yayin::${msg}`);
}

function ensureDir(dir) {
  mkdirSync(dir, { recursive: true });
}

function walk(dir) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

/** Sunucudaki mevcut JSON'u okur. Basarisiz olursa null (cagiran YAZMAMALI). */
async function fetchJson(path) {
  const url = `${BASE_URL}/${path}`;
  try {
    const res = await fetch(url, {
      signal: AbortSignal.timeout(20000),
      headers: { 'user-agent': 'bogahost-native-ci', 'cache-control': 'no-cache' },
    });
    if (res.status === 404) return {}; // ilk yayin — bos durum normaldir
    if (!res.ok) {
      warn(`${url} okunamadi (HTTP ${res.status}). Cloudflare grey-cloud/bypass kurali gerekebilir (docs/UPDATE.md).`);
      return null;
    }
    return await res.json();
  } catch (e) {
    warn(`${url} okunamadi (${e.message}).`);
    return null;
  }
}

// ---------------------------------------------------------------------------
// 1) Artifact'lari uygulamaya gore topla
// ---------------------------------------------------------------------------

/** dist/<workflow>-<app>/... yolundan app anahtarini cikarir. */
function appOf(file) {
  const parts = resolve(file).split(/[/\\]/);
  for (const part of parts) {
    for (const app of APPS) {
      if (part === app || part.endsWith(`-${app}`)) return app;
    }
  }
  // Yedek: dosya adindan tahmin et.
  const lower = basename(file).toLowerCase();
  return APPS.find((a) => lower.includes(a)) ?? null;
}

const files = walk(ARTIFACTS).filter((f) => !f.endsWith('.sig'));
if (files.length === 0) warn(`${ARTIFACTS} altinda artifact bulunamadi.`);

const byApp = Object.fromEntries(APPS.map((a) => [a, []]));
for (const f of files) {
  const app = appOf(f);
  if (app) byApp[app].push(f);
}

/** macOS build job'unun yazdigi mimari listesi (universal ise iki deger). */
function macArchs(app) {
  const marker = walk(ARTIFACTS).find(
    (f) => basename(f) === 'updater-archs.txt' && appOf(f) === app
  );
  if (!marker) return ['aarch64'];
  const archs = readFileSync(marker, 'utf8').trim().split(/\s+/).filter(Boolean);
  return archs.length ? archs : ['aarch64'];
}

// ---------------------------------------------------------------------------
// 2) Dosyalari yeniden adlandirip downloads/ altina kopyala
// ---------------------------------------------------------------------------
// Orijinal adlar bosluk iceriyor ("Bogahost Finans_1.1.0_x64-setup.exe");
// URL/FTP sorunlarini onlemek icin guvenli slug'a cevriliyor.

const downloadsDir = join(OUT, 'downloads');
ensureDir(downloadsDir);

/** { app: { platformKey: {file, sig} }, ... } — updater icin. */
const updaterAssets = {};
/** index.html icin: { app: [ {platform, label, file, size} ] } */
const downloadEntries = {};

function publishFile(app, srcPath, targetName, { platformKey = null, label = null } = {}) {
  const dest = join(downloadsDir, targetName);
  copyFileSync(srcPath, dest);

  let signature = null;
  const sigPath = `${srcPath}.sig`;
  if (existsSync(sigPath)) {
    signature = readFileSync(sigPath, 'utf8').trim();
    copyFileSync(sigPath, `${dest}.sig`);
  }

  (downloadEntries[app] ??= []).push({
    platform: platformKey ?? PLATFORM,
    surum: VERSION,
    label: label ?? targetName,
    file: targetName,
    url: `${BASE_URL}/downloads/${targetName}`,
    size: statSync(srcPath).size,
  });

  return { file: targetName, url: `${BASE_URL}/downloads/${targetName}`, signature };
}

for (const app of APPS) {
  const appFiles = byApp[app];
  if (!appFiles.length) continue;
  updaterAssets[app] = {};

  if (PLATFORM === 'windows') {
    const nsis = appFiles.find((f) => f.toLowerCase().endsWith('.exe'));
    const msi = appFiles.find((f) => f.toLowerCase().endsWith('.msi'));

    let winEntry = null;
    if (nsis) {
      winEntry = publishFile(app, nsis, `${app}-${VERSION}-windows-x86_64-setup.exe`, {
        platformKey: 'windows-x86_64',
        label: 'Windows (.exe kurulum)',
      });
    }
    if (msi) {
      const msiEntry = publishFile(app, msi, `${app}-${VERSION}-windows-x86_64.msi`, {
        platformKey: 'windows-x86_64',
        label: 'Windows (.msi)',
      });
      // Updater icin IMZALI olani tercih et.
      if (!winEntry?.signature && msiEntry.signature) winEntry = msiEntry;
    }

    if (winEntry?.signature) {
      updaterAssets[app]['windows-x86_64'] = winEntry;
    } else if (winEntry) {
      warn(`${app}: Windows installer'in .sig dosyasi yok — otomatik guncelleme YAYINLANMADI (yalnizca indirme).`);
    }
  }

  if (PLATFORM === 'macos') {
    const archs = macArchs(app);
    const tar = appFiles.find((f) => f.endsWith('.app.tar.gz'));
    const dmg = appFiles.find((f) => f.toLowerCase().endsWith('.dmg'));

    if (dmg) {
      publishFile(app, dmg, `${app}-${VERSION}-macos.dmg`, {
        platformKey: 'darwin',
        label: archs.length > 1 ? 'macOS (.dmg, Universal)' : 'macOS (.dmg, Apple Silicon)',
      });
    }

    if (tar) {
      const entry = publishFile(app, tar, `${app}-${VERSION}-darwin-${archs.join('-')}.app.tar.gz`, {
        platformKey: 'darwin-updater',
        label: 'macOS (otomatik güncelleme paketi)',
      });
      if (entry.signature) {
        // Universal binary ise ayni paket her iki mimariye de sunulur.
        for (const arch of archs) updaterAssets[app][`darwin-${arch}`] = entry;
      } else {
        warn(`${app}: .app.tar.gz.sig yok — macOS otomatik guncelleme YAYINLANMADI.`);
      }
    } else {
      warn(`${app}: .app.tar.gz bulunamadi (createUpdaterArtifacts kapali olabilir) — macOS otomatik guncelleme YAYINLANMADI.`);
    }
  }

  if (PLATFORM === 'android') {
    // Android'de OTOMATIK guncelleme YOKTUR; yalnizca indirme linki yayinlanir.
    const apk = appFiles.find((f) => f.toLowerCase().endsWith('.apk'));
    if (apk) {
      publishFile(app, apk, `${app}-${VERSION}-android.apk`, {
        platformKey: 'android',
        label: 'Android (.apk)',
      });
    }
  }
}

// ---------------------------------------------------------------------------
// 3) Updater manifestleri
// ---------------------------------------------------------------------------

function manifest(platforms) {
  return { version: VERSION, notes: NOTES, pub_date: PUB_DATE, platforms };
}

function writeJson(relPath, data) {
  const dest = join(OUT, relPath);
  ensureDir(resolve(dest, '..'));
  writeFileSync(dest, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
  console.log(`  + ${relPath}`);
}

for (const app of APPS) {
  const platforms = updaterAssets[app] ?? {};
  const keys = Object.keys(platforms);
  if (!keys.length) continue;

  // 3a) BIRINCIL: per-target/arch dosyalari (cakisma riski yok).
  for (const key of keys) {
    const [target, arch] = key.split('-');
    const { url, signature } = platforms[key];
    writeJson(`updates/${app}/${target}/${arch}/latest.json`, manifest({ [key]: { signature, url } }));
  }

  // 3b) YEDEK: birlesik dosya — yalnizca sunucudaki kopya OKUNABILDIYSE yazilir.
  const existing = await fetchJson(`updates/${app}/latest.json`);
  if (existing === null) {
    warn(`${app}: birlesik updates/${app}/latest.json guncellenmedi (sunucudaki kopya okunamadi). Per-arch endpoint'ler yayinlandi, otomatik guncelleme CALISIR.`);
    continue;
  }
  // Ayni surumse diger platformlarin kayitlarini KORU; yeni surumse sifirdan basla.
  const keep = existing.version === VERSION ? (existing.platforms ?? {}) : {};
  const merged = { ...keep };
  for (const key of keys) merged[key] = { signature: platforms[key].signature, url: platforms[key].url };
  writeJson(`updates/${app}/latest.json`, manifest(merged));
}

// ---------------------------------------------------------------------------
// 4) ESKI manifest (v1.1.0 istemcileri bu adresi okur — KORUNUYOR)
// ---------------------------------------------------------------------------
// https://bogahost.com/native/latest.json  ==  https://native.bogahost.com/latest.json
writeJson('latest.json', {
  version: VERSION,
  notes: NOTES,
  url: `${BASE_URL}/`,
});

// ---------------------------------------------------------------------------
// 5) downloads.json + index.html
// ---------------------------------------------------------------------------

const state = await fetchJson('updates/downloads.json');
if (state === null) {
  warn('updates/downloads.json okunamadi — index.html YENIDEN URETILMEDI (sunucudaki kopya korunuyor).');
} else {
  const apps = state.apps ?? {};
  for (const app of APPS) {
    const fresh = downloadEntries[app] ?? [];
    if (!fresh.length) continue;
    // Bu uygulamanin BU platformdaki eski kayitlari yenileriyle degisir;
    // diger platformlari ve diger uygulamalar oldugu gibi kalir.
    const others = (apps[app] ?? []).filter((e) => !platformGroup(e.platform, PLATFORM));
    apps[app] = [...others, ...fresh];
  }
  const nextState = { version: VERSION, notes: NOTES, updated: PUB_DATE, apps };
  writeJson('updates/downloads.json', nextState);
  const indexHtml = renderIndex();
  if (indexHtml) {
    writeFileSync(join(OUT, 'index.html'), indexHtml, 'utf8');
    console.log('  + index.html (sablondan)');
  }
}

/** Bir kaydin, su an yayinlanan platform grubuna ait olup olmadigi. */
function platformGroup(entryPlatform, current) {
  const p = String(entryPlatform ?? '');
  if (current === 'windows') return p.startsWith('windows');
  if (current === 'macos') return p.startsWith('darwin') || p.startsWith('macos');
  if (current === 'android') return p.startsWith('android');
  return false;
}

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}

function mb(bytes) {
  return `${(Number(bytes || 0) / 1048576).toFixed(1)} MB`;
}

/**
 * index.html — SABIT sablon: scripts/ci/index.template.html
 * Sayfa dosya listesini CALISMA ANINDA /updates/downloads.json'dan okur; bu yuzden
 * sunucu tarafinda veri enjekte etmeye GEREK YOKTUR. Tasarim degisikligi icin yalnizca
 * index.template.html duzenlenir — CI her yayinda onu aynen kopyalar.
 * (Once sablon yoktu ve her yayin sunucudaki premium tasarimi eziyordu.)
 */
function renderIndex() {
  const tpl = join(HERE, 'index.template.html');
  if (!existsSync(tpl)) {
    warn('scripts/ci/index.template.html YOK — index.html uretilmedi, sunucudaki korunuyor.');
    return null;
  }
  return readFileSync(tpl, 'utf8');
}

console.log(`\nHazir: ${OUT} (surum ${VERSION}, platform ${PLATFORM})`);
if (warnings.length) console.log(`Uyari sayisi: ${warnings.length}`);
