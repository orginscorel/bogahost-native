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

// UYGULAMA BAZLI TUTARLILIK.
//
// Eskiden altı uygulamanın da kök package.json ile AYNI sürümde olması
// şart koşuluyordu. Artık her itmede yalnızca değişen uygulama derleniyor;
// tek uygulamayı güncellemek için altısının birden sürümünü oynatmak, altısını
// birden yeniden derletirdi — yani seçmeli derlemenin bütün anlamını yok ederdi.
//
// Aranan şey artık şu: BİR uygulamanın kendi dosyaları birbiriyle tutuyor mu?
// Sapma burada sessizdir ve zararlıdır: Cargo.toml arayüzde gösterilen sürümü,
// tauri.conf.json ise paketin ve güncelleyicinin gördüğü sürümü belirler.
// İkisi ayrışırsa kullanıcı "güncelledim ama sürüm değişmedi" der.
const problems = [];

const semver = (v) => typeof v === 'string' && /^\d+\.\d+\.\d+$/.test(v);

for (const app of APPS) {
  const confPath = join(ROOT, 'tauri', app, 'src-tauri', 'tauri.conf.json');
  const cargoPath = join(ROOT, 'tauri', app, 'src-tauri', 'Cargo.toml');
  const pkgPath = join(ROOT, 'tauri', app, 'package.json');
  if (!existsSync(confPath) && !existsSync(cargoPath)) continue;

  // YALNIZ İKİ DOSYA BAĞLAYICI:
  //   Cargo.toml      → arayüzde gösterilen sürüm (app.getVersion)
  //   tauri.conf.json → paketin ve güncelleyicinin gördüğü sürüm
  // tauri/<app>/package.json yalnızca Tauri CLI'nin npm meta verisidir; hiçbir
  // yerde kullanıcıya görünmez. Uzun süredir 1.6.0'da kalmış ve bir zararı
  // olmamış — beş uygulamayı sırf bu yüzden yeniden derletmenin anlamı yok.
  // Yine de sapıyorsa uyarı basıyoruz ki fark edilsin.
  const bulunan = {};
  if (existsSync(confPath)) bulunan['tauri.conf.json'] = readJson(confPath).version;
  if (existsSync(cargoPath)) bulunan['Cargo.toml'] = cargoVersion(cargoPath);

  if (existsSync(pkgPath)) {
    const pv = readJson(pkgPath).version;
    const ana = bulunan['Cargo.toml'] ?? bulunan['tauri.conf.json'];
    if (pv !== ana) {
      console.log(`::notice title=Surum notu::${app}/package.json=${pv}, paket surumu=${ana} (zararsiz, meta veri)`);
    }
  }

  const degerler = [...new Set(Object.values(bulunan))];
  if (degerler.length > 1) {
    const detay = Object.entries(bulunan).map(([k, v]) => `${k}=${v}`).join(', ');
    problems.push(
      `${app}: dosyalar ayrışıyor (${detay}) ` +
        '— Cargo.toml arayuzde gosterilen, tauri.conf.json guncelleyicinin gordugu surumdur'
    );
  } else if (!semver(degerler[0])) {
    problems.push(`${app}: gecersiz surum "${degerler[0]}" (x.y.z olmali)`);
  } else {
    console.log(`[surum] ${app}: ${degerler[0]}`);
  }
}
if (problems.length) {
  console.error('::error title=Surum uyusmazligi::Bir uygulamanin dosyalari birbiriyle tutmuyor:');
  for (const p of problems) console.error(`  ✗ ${p}`);
  console.error('\nO uygulamanin uc dosyasini ayni surume getirin; aksi halde kurulan paket ile gosterilen surum farkli olur.');
  process.exit(1);
}

console.log('[surum] her uygulama kendi icinde tutarli.');
