#!/usr/bin/env node
// Surum tutarlilik denetimi — CI'da her build oncesi calisir.
//
// NEDEN VAR?
//   Tauri'de surum IKI ayri yerden okunur:
//     * `tauri.conf.json` -> paketleme/installer surumu + updater karsilastirmasi
//     * `Cargo.toml`      -> `env!("CARGO_PKG_VERSION")` -> ARAYUZDE GOSTERILEN surum
//                            (sidebar, tepsi menusu, "Hakkinda" penceresi)
//
//   v1.7.0 ve v1.8.0'da `Cargo.toml` bump'i atlandi. Sonuc: 1.8.0 paketi dogru
//   indirilip kuruldu, ama uygulama kendini 1.6.0 sanmaya devam etti. Kullanici
//   "guncelleme atti diyor ama kurulu surum degismiyor" seklinde bildirdi —
//   yani hata SESSIZDI, hicbir yerde patlamadi.
//
//   Bu betik o sessiz sapmayi imkansiz kilar: surumler ayrisirsa CI KIRMIZI yanar
//   ve yanlis etiketli bir surum hic yayinlanmaz.

import { readFileSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const APPS = ['finans', 'dcim', 'chat', 'task', 'muh', 'kasa'];

const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

/** Cargo.toml'daki ILK `version = "..."` satiri = paketin kendi surumu.
 *  (Bagimlilik surumleri daha asagida gelir, onlara bakmiyoruz.) */
function cargoVersion(path) {
  const text = readFileSync(path, 'utf8');
  const m = text.match(/^\s*version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
}

const expected = readJson(join(ROOT, 'package.json')).version;
const problems = [];

const appsCfgPath = join(ROOT, 'apps.config.json');
if (existsSync(appsCfgPath)) {
  const v = readJson(appsCfgPath).version;
  if (v !== expected) problems.push(`apps.config.json: ${v} (beklenen ${expected})`);
}

for (const app of APPS) {
  const confPath = join(ROOT, 'tauri', app, 'src-tauri', 'tauri.conf.json');
  const cargoPath = join(ROOT, 'tauri', app, 'src-tauri', 'Cargo.toml');

  if (existsSync(confPath)) {
    const v = readJson(confPath).version;
    if (v !== expected) problems.push(`tauri/${app}/src-tauri/tauri.conf.json: ${v} (beklenen ${expected})`);
  }
  if (existsSync(cargoPath)) {
    const v = cargoVersion(cargoPath);
    if (v !== expected) {
      problems.push(
        `tauri/${app}/src-tauri/Cargo.toml: ${v} (beklenen ${expected}) ` +
          '— ARAYUZDE GOSTERILEN surum burasidir, sapma sessizdir'
      );
    }
  }
}

if (problems.length) {
  console.error(`::error title=Surum uyusmazligi::Beklenen surum ${expected}, asagidakiler sapiyor:`);
  for (const p of problems) console.error(`  ✗ ${p}`);
  console.error('\nTumunu ayni surume getirin; aksi halde kurulan paket ile gosterilen surum farkli olur.');
  process.exit(1);
}

console.log(`[surum] tum dosyalar tutarli: ${expected}`);
