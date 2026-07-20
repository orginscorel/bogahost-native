#!/usr/bin/env node
/**
 * Bogahost native — Capacitor platform HAZIRLAMA adımı.
 *
 * Kullanım:
 *   node scripts/prepare.mjs --app <key> --platform <android|ios>
 *
 * Taze bir `git clone`'da `capacitor/<key>/android` ve `/ios` platform klasörleri
 * YOKTUR (commit edilmez). Bu betik, `npx cap sync`'ten ÖNCE çalışır ve şunları yapar:
 *
 *   1. Platform yoksa `npx cap add <platform>` ile üretir (idempotent — varsa atlar).
 *   2. `npm run icons` (gen-icons) çıktısını üretilen platforma yerleştirir
 *      (Android res/mipmap-*, iOS AppIcon.appiconset). resources/ yoksa üretmeyi dener.
 *   3. `android-overrides/` ve `ios-overrides/` dosyalarını üretilen projeye kopyalar:
 *        - AndroidManifest.xml  → tam dosya değişimi (override, Capacitor 7 varsayılanının
 *          bir ÜST-KÜMESİ olacak biçimde yazılmıştır: .MainActivity + FileProvider korunur,
 *          izinler/networkSecurityConfig/App-Link intent-filter EKLENİR).
 *        - res/xml/network_security_config.xml, res/mipmap-anydpi-v26/*.xml → kopyalanır.
 *        - res/values/colors.xml → BİRLEŞTİRİLİR (union; override kazanır).
 *        - iOS Info.plist → PlistBuddy ile anahtar-birleştirme (macOS runner'da mevcut).
 *        - iOS App.entitlements → yerleştirilir VE Xcode projesine
 *          CODE_SIGN_ENTITLEMENTS build ayarı olarak BAĞLANIR (bkz. patchPbxproj).
 *
 *   4. SÜRÜM alanlarını yazar (mağaza şartı):
 *        - iOS   : MARKETING_VERSION + CURRENT_PROJECT_VERSION (pbxproj)
 *        - Android: versionName + versionCode (app/build.gradle)
 *      Sürüm = kök package.json "version". Build numarası = BUILD_NUMBER ortam
 *      değişkeni (CI'da github.run_number) + BUILD_NUMBER_OFFSET; yoksa sürümden türetilir.
 *
 *   assetlinks.json / apple-app-site-association SUNUCU dosyalarıdır (canlı .well-known
 *   altına ELLE konur) — uygulama paketine KOPYALANMAZ; bilinçli olarak atlanır.
 *
 * Adım seçimi (isteğe bağlı):
 *   --step all        (varsayılan) tüm hazırlık
 *   --step xcodeproj  yalnız pbxproj yamaları (entitlements + sürüm).
 *                     `npx cap sync ios` sonrası GÜVENLİK AĞI olarak ikinci kez çalıştırılır.
 *
 * Idempotent: iki kez çalışınca bozmaz. Toolchain (cap/PlistBuddy) yoksa NET hata + çıkış 1.
 * Node 20+, sadece stdlib (+ isteğe bağlı sharp, gen-icons üzerinden).
 */
import { spawnSync } from 'node:child_process';
import {
  readFileSync,
  writeFileSync,
  existsSync,
  mkdirSync,
  cpSync,
  readdirSync,
  rmSync,
} from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');

const ANDROID_DPIS = ['mdpi', 'hdpi', 'xhdpi', 'xxhdpi', 'xxxhdpi'];

// ───────────────────────── yardımcılar ─────────────────────────

function die(msg) {
  console.error(`✗ ${msg}`);
  process.exit(1);
}

function parseArgs(argv) {
  const out = { app: null, platform: null, step: 'all' };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--app') out.app = argv[++i];
    else if (a.startsWith('--app=')) out.app = a.split('=')[1];
    else if (a === '--platform') out.platform = argv[++i];
    else if (a.startsWith('--platform=')) out.platform = a.split('=')[1];
    else if (a === '--step') out.step = argv[++i];
    else if (a.startsWith('--step=')) out.step = a.split('=')[1];
  }
  return out;
}

// ───────────────────────── sürüm / build numarası ─────────────────────────

/** Kök package.json "version" → "1.6.0". Mağazaya görünen sürüm budur. */
function rootVersion() {
  const v = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version;
  if (!/^\d+\.\d+\.\d+/.test(String(v || ''))) {
    die(`package.json "version" semver değil: ${JSON.stringify(v)}`);
  }
  return String(v).split('-')[0]; // ön-sürüm etiketi mağaza alanlarında geçersiz
}

/**
 * Artan tamsayı build numarası.
 *  - CI: BUILD_NUMBER=github.run_number (workflow başına monoton artar) [+ BUILD_NUMBER_OFFSET]
 *  - Yerel/fallback: sürümden türetilir (1.6.0 → 10600) — mağazaya yerelden yükleme yapılmaz.
 * App Store ve Play, her yüklemede ÖNCEKİNDEN BÜYÜK bir sayı ister.
 */
function buildNumber(version) {
  const raw = parseInt(process.env.BUILD_NUMBER || '', 10);
  const off = parseInt(process.env.BUILD_NUMBER_OFFSET || '0', 10) || 0;
  if (Number.isInteger(raw) && raw > 0) return String(raw + off);
  const [ma, mi, pa] = version.split('.').map((n) => parseInt(n, 10) || 0);
  return String(ma * 10000 + mi * 100 + pa);
}

function loadApp(key) {
  const cfg = JSON.parse(readFileSync(join(ROOT, 'apps.config.json'), 'utf8'));
  const app = (cfg.apps || []).find((a) => a.key === key);
  if (!app) {
    die(
      `Bilinmeyen uygulama: "${key}". Geçerli: ${(cfg.apps || [])
        .map((a) => a.key)
        .join(', ')}`
    );
  }
  return app;
}

/** Komutu çalıştırır; başarısızsa (allowFail değilse) süreci sonlandırır. */
function sh(cmd, args, cwd, { allowFail = false } = {}) {
  console.log(`  $ ${cmd} ${args.join(' ')}${cwd ? `   (cwd: ${cwd})` : ''}`);
  const res = spawnSync(cmd, args, { cwd, stdio: 'inherit' });
  if (res.error) {
    if (allowFail) return false;
    if (res.error.code === 'ENOENT') {
      die(`"${cmd}" bulunamadı — bu adım için toolchain kurulu değil.`);
    }
    die(`Komut hatası: ${res.error.message}`);
  }
  if (res.status !== 0) {
    if (allowFail) return false;
    die(`"${cmd} ${args.join(' ')}" çıkış kodu ${res.status}.`);
  }
  return true;
}

function copyInto(src, destDir, destName) {
  if (!existsSync(src)) {
    console.log(`  ↷ atlandı (kaynak yok): ${src}`);
    return;
  }
  mkdirSync(destDir, { recursive: true });
  const dest = join(destDir, destName);
  cpSync(src, dest, { recursive: false });
  console.log(`  ✓ ${destName} → ${destDir}`);
}

/** gen-icons çıktısı (resources/) yoksa üretmeyi dener. sharp yoksa uyarı, çökmez. */
function ensureResources(app, subdir) {
  const target = join(ROOT, 'capacitor', app.key, 'resources', subdir);
  if (existsSync(target)) return true;
  console.log(`  ℹ resources/${subdir} yok — gen-icons çalıştırılıyor…`);
  sh('node', [join('scripts', 'gen-icons.mjs'), '--app', app.key], ROOT, {
    allowFail: true,
  });
  if (!existsSync(target)) {
    console.log(
      `  ⚠ resources/${subdir} üretilemedi (sharp kurulu değil?). ` +
        `Capacitor'ın VARSAYILAN ikonları kullanılacak — build yine de geçer.`
    );
    return false;
  }
  return true;
}

// ───────────────────────── colors.xml birleştirme ─────────────────────────

function mergeColorsXml(overrideFile, targetFile) {
  if (!existsSync(overrideFile)) return;
  const colorRe = /<color\s+name="([^"]+)"\s*>([^<]*)<\/color>/g;

  const overrideXml = readFileSync(overrideFile, 'utf8');
  const merged = new Map(); // name → value (override kazanır, ama önce mevcut yüklenir)

  if (existsSync(targetFile)) {
    const cur = readFileSync(targetFile, 'utf8');
    let m;
    while ((m = colorRe.exec(cur)) !== null) merged.set(m[1], m[2]);
  }
  let m;
  colorRe.lastIndex = 0;
  while ((m = colorRe.exec(overrideXml)) !== null) merged.set(m[1], m[2]);

  const lines = ['<?xml version="1.0" encoding="utf-8"?>', '<resources>'];
  for (const [name, val] of merged) {
    lines.push(`    <color name="${name}">${val}</color>`);
  }
  lines.push('</resources>', '');
  mkdirSync(dirname(targetFile), { recursive: true });
  writeFileSync(targetFile, lines.join('\n'));
  console.log(`  ✓ colors.xml birleştirildi (${merged.size} renk) → ${targetFile}`);

  // Capacitor bazı renkleri AYRI dosyada üretir (ör. res/values/ic_launcher_background.xml).
  // Aynı renk hem orada hem colors.xml'de tanımlıysa Gradle "Duplicate resources" ile PATLAR.
  // colors.xml artık tek doğru kaynak → kardeş dosyalardaki aynı isimli renkleri temizle.
  const valuesDir = dirname(targetFile);
  for (const f of readdirSync(valuesDir)) {
    if (!f.endsWith('.xml') || f === 'colors.xml') continue;
    const p = join(valuesDir, f);
    let xml = readFileSync(p, 'utf8');
    let changed = false;
    xml = xml.replace(
      /[ \t]*<color\s+name="([^"]+)"\s*>[^<]*<\/color>[ \t]*\r?\n?/g,
      (full, name) => (merged.has(name) ? ((changed = true), '') : full)
    );
    if (!changed) continue;
    if (!/<(color|string|dimen|style|bool|integer|array|item|declare-styleable)\b/.test(xml)) {
      rmSync(p);
      console.log(`  ✓ yinelenen renk dosyası silindi → ${f}`);
    } else {
      writeFileSync(p, xml);
      console.log(`  ✓ ${f} içinden yinelenen renk(ler) temizlendi`);
    }
  }
}

// ───────────────────────── ANDROID sürüm alanları ─────────────────────────

/**
 * android/app/build.gradle içindeki versionName + versionCode alanlarını yazar.
 * Capacitor şablonu bunları `defaultConfig` içinde `versionCode 1` / `versionName "1.0"`
 * olarak üretir. Play Console her yüklemede versionCode'un ARTMASINI şart koşar.
 * Idempotent: değer zaten doğruysa dosya değişmez.
 */
function patchAndroidVersion(app) {
  const gradle = join(ROOT, 'capacitor', app.key, 'android', 'app', 'build.gradle');
  if (!existsSync(gradle)) {
    die(`build.gradle bulunamadı: ${gradle} (cap add android başarısız olmuş olabilir).`);
  }
  const version = rootVersion();
  const code = buildNumber(version);

  let src = readFileSync(gradle, 'utf8');
  const before = src;

  // `versionCode 1` veya `versionCode = 1` (AGP 8 Kotlin/Groovy DSL varyantları)
  const codeRe = /^([ \t]*versionCode[ \t]*=?[ \t]*)(\d+)([ \t\r]*)$/m;
  const nameRe = /^([ \t]*versionName[ \t]*=?[ \t]*)(["'])[^"']*\2([ \t\r]*)$/m;

  if (!codeRe.test(src)) die(`build.gradle içinde "versionCode" satırı bulunamadı: ${gradle}`);
  if (!nameRe.test(src)) die(`build.gradle içinde "versionName" satırı bulunamadı: ${gradle}`);

  src = src.replace(codeRe, (_m, p1, _old, p3) => `${p1}${code}${p3}`);
  src = src.replace(nameRe, (_m, p1, q, p3) => `${p1}${q}${version}${q}${p3}`);

  if (src !== before) writeFileSync(gradle, src);
  console.log(`  ✓ Android sürüm: versionName="${version}", versionCode=${code}`);
}

// ───────────────────────── ANDROID ─────────────────────────

function prepareAndroid(app) {
  const appDir = join(ROOT, 'capacitor', app.key);
  const androidDir = join(appDir, 'android');
  const overrides = join(appDir, 'android-overrides');

  if (!existsSync(androidDir)) {
    console.log(`▶ [${app.key}] android platformu yok — npx cap add android`);
    sh('npx', ['--no-install', 'cap', 'add', 'android'], appDir);
  } else {
    console.log(`▶ [${app.key}] android platformu mevcut — cap add atlanıyor`);
  }
  if (!existsSync(androidDir)) {
    die(`android/ üretilemedi (${app.key}). @capacitor/android kurulu mu? (per-app npm install)`);
  }

  const mainDir = join(androidDir, 'app', 'src', 'main');
  const resDir = join(mainDir, 'res');

  // 1) AndroidManifest — tam dosya değişimi (override = varsayılanın üst-kümesi).
  copyInto(join(overrides, 'AndroidManifest.xml'), mainDir, 'AndroidManifest.xml');

  // 2) network_security_config.xml
  copyInto(
    join(overrides, 'res', 'xml', 'network_security_config.xml'),
    join(resDir, 'xml'),
    'network_security_config.xml'
  );

  // 3) FileProvider güvenlik ağı: manifest @xml/file_paths'a başvurur.
  //    Capacitor 7 şablonu bunu zaten üretir; yoksa minimal bir tane yaz.
  const filePaths = join(resDir, 'xml', 'file_paths.xml');
  if (!existsSync(filePaths)) {
    mkdirSync(dirname(filePaths), { recursive: true });
    writeFileSync(
      filePaths,
      '<?xml version="1.0" encoding="utf-8"?>\n' +
        '<paths xmlns:android="http://schemas.android.com/apk/res/android">\n' +
        '    <cache-path name="my_cache_images" path="." />\n' +
        '    <external-path name="my_images" path="." />\n' +
        '    <files-path name="my_files" path="." />\n' +
        '</paths>\n'
    );
    console.log(`  ✓ file_paths.xml (güvenlik ağı) yazıldı`);
  }

  // 4) Adaptive icon XML'leri (anydpi-v26).
  const anydpiSrc = join(overrides, 'res', 'mipmap-anydpi-v26');
  if (existsSync(anydpiSrc)) {
    const anydpiDest = join(resDir, 'mipmap-anydpi-v26');
    for (const f of readdirSync(anydpiSrc)) copyInto(join(anydpiSrc, f), anydpiDest, f);
  }

  // 5) colors.xml BİRLEŞTİR.
  mergeColorsXml(
    join(overrides, 'res', 'values', 'colors.xml'),
    join(resDir, 'values', 'colors.xml')
  );

  // 6) İkon PNG'lerini yerleştir (varsa).
  if (ensureResources(app, 'android')) {
    const genRoot = join(appDir, 'resources', 'android');
    for (const dpi of ANDROID_DPIS) {
      const srcDir = join(genRoot, `mipmap-${dpi}`);
      if (!existsSync(srcDir)) continue;
      const destDir = join(resDir, `mipmap-${dpi}`);
      for (const f of readdirSync(srcDir)) copyInto(join(srcDir, f), destDir, f);
    }
  }

  // 7) Sürüm alanları (mağaza şartı).
  patchAndroidVersion(app);

  console.log(`✓ [${app.key}] android hazırlandı (${app.url}).`);
}

// ───────────────────────── iOS plist parçası birleştirme ─────────────────────────

function decodeEntities(s) {
  return s
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&');
}

/** Minimal plist XML → JS ağacı (string/bool/int/real/array/dict). dict = {__dict, entries:[[k,v]]}. */
function parsePlist(xml) {
  let s = xml
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<\?xml[\s\S]*?\?>/g, '')
    .replace(/<!DOCTYPE[^>]*>/g, '');
  const m = s.match(/<plist[^>]*>([\s\S]*)<\/plist>/);
  s = m ? m[1] : s;
  let pos = 0;

  const skipWs = () => {
    while (pos < s.length && /\s/.test(s[pos])) pos++;
  };
  const readOpenTag = () => {
    // pos, '<' üzerinde. {name, selfClose} döner.
    const end = s.indexOf('>', pos);
    if (end < 0) throw new Error('Bozuk plist: kapanmayan tag');
    let raw = s.slice(pos + 1, end);
    pos = end + 1;
    const selfClose = raw.endsWith('/');
    if (selfClose) raw = raw.slice(0, -1);
    return { name: raw.trim().split(/\s+/)[0], selfClose };
  };
  const readTextUntil = (closeTag) => {
    const e = s.indexOf(closeTag, pos);
    if (e < 0) throw new Error(`Bozuk plist: ${closeTag} yok`);
    const v = s.slice(pos, e);
    pos = e + closeTag.length;
    return v;
  };

  function parseValue() {
    skipWs();
    if (s[pos] !== '<') throw new Error(`Bozuk plist @${pos}: değer beklendi`);
    const tag = readOpenTag();
    switch (tag.name) {
      case 'true':
        return true;
      case 'false':
        return false;
      case 'string':
        return decodeEntities(readTextUntil('</string>'));
      case 'integer':
        return parseInt(readTextUntil('</integer>').trim(), 10);
      case 'real':
        return parseFloat(readTextUntil('</real>').trim());
      case 'array': {
        const arr = [];
        for (;;) {
          skipWs();
          if (s.startsWith('</array>', pos)) {
            pos += '</array>'.length;
            break;
          }
          arr.push(parseValue());
        }
        return arr;
      }
      case 'dict': {
        const entries = [];
        for (;;) {
          skipWs();
          if (s.startsWith('</dict>', pos)) {
            pos += '</dict>'.length;
            break;
          }
          const kt = readOpenTag();
          if (kt.name !== 'key') throw new Error(`dict içinde <key> beklendi, "${kt.name}" bulundu`);
          const key = decodeEntities(readTextUntil('</key>'));
          entries.push([key, parseValue()]);
        }
        return { __dict: true, entries };
      }
      default:
        throw new Error(`Beklenmeyen plist tag: <${tag.name}>`);
    }
  }

  skipWs();
  return parseValue();
}

/** PlistBuddy "Add" komut listesi üretir (path, type, value). */
function plistAddCommands(path, val, cmds) {
  if (val === true || val === false) {
    cmds.push(`Add ${path} bool ${val}`);
  } else if (typeof val === 'number') {
    cmds.push(`Add ${path} ${Number.isInteger(val) ? 'integer' : 'real'} ${val}`);
  } else if (typeof val === 'string') {
    cmds.push(`Add ${path} string ${val}`);
  } else if (Array.isArray(val)) {
    cmds.push(`Add ${path} array`);
    val.forEach((v, i) => plistAddCommands(`${path}:${i}`, v, cmds));
  } else if (val && val.__dict) {
    cmds.push(`Add ${path} dict`);
    for (const [k, v] of val.entries) plistAddCommands(`${path}:${k}`, v, cmds);
  } else {
    throw new Error(`Desteklenmeyen plist değeri: ${JSON.stringify(val)}`);
  }
}

function mergeInfoPlist(partialFile, targetPlist) {
  const PB = '/usr/libexec/PlistBuddy';
  if (!existsSync(PB)) {
    die(`PlistBuddy bulunamadı (${PB}). iOS hazırlama yalnızca macOS runner'da çalışır.`);
  }
  if (!existsSync(targetPlist)) {
    die(`Hedef Info.plist yok: ${targetPlist} (cap add ios başarısız olmuş olabilir).`);
  }
  const tree = parsePlist(readFileSync(partialFile, 'utf8'));
  if (!tree || !tree.__dict) die(`plist parçası kök <dict> içermiyor: ${partialFile}`);

  for (const [key, val] of tree.entries) {
    // Idempotent: önce sil (yoksa hata yut), sonra taze ekle.
    spawnSync(PB, ['-c', `Delete :${key}`, targetPlist], { stdio: 'ignore' });
    const cmds = [];
    plistAddCommands(`:${key}`, val, cmds);
    for (const c of cmds) {
      const r = spawnSync(PB, ['-c', c, targetPlist], { stdio: 'inherit' });
      if (r.status !== 0) die(`PlistBuddy başarısız: ${c}`);
    }
    console.log(`  ✓ Info.plist anahtarı birleştirildi: ${key}`);
  }
}

// ───────────────────────── iOS: project.pbxproj yamaları ─────────────────────────
//
// NEDEN GEREKLİ: prepare, App.entitlements dosyasını yerine koyar; ama Xcode bir
// entitlements dosyasını YALNIZCA hedefin CODE_SIGN_ENTITLEMENTS build ayarı ona
// işaret ediyorsa imzaya gömer. Ayar yoksa imzasız doğrulama build'i yine geçer
// (entitlements zaten uygulanmaz) — fakat İMZALI IPA'da associated-domains
// (universal link) ve aps-environment (push) SESSİZCE devre dışı kalır.
//
// NEDEN GÜVENLİ:
//  • Yalnız `buildSettings = { … }` bloklarına dokunur; blok sınırı süslü parantez
//    sayarak (tırnak içi atlanarak) bulunur — regex ile "yaklaşık" eşleşme yapılmaz.
//  • Yalnız PRODUCT_BUNDLE_IDENTIFIER içeren bloklar hedeflenir → uygulama hedefinin
//    Debug/Release konfigürasyonları. Proje-düzeyi bloklara dokunulmaz.
//  • Var olan satır varsa DEĞİŞTİRİLİR, yoksa EKLENİR → idempotent (ikinci çalıştırma
//    aynı içeriği üretir, dosya büyümez).
//  • Tüm değerler tırnaklanır (pbxproj'da tırnaklı string her zaman geçerlidir).
//  • Yazmadan önce doğrulama: parantez dengesi + hiç blok eşleşmediyse NET hata ile çıkış
//    (sessizce yetkisiz IPA üretmektense CI'ın kırmızı yanması yeğdir).

/** `buildSettings = {` bloklarının [{start:'{' idx, end:'}' idx}] listesi. */
function findBuildSettingsBlocks(src) {
  const needle = 'buildSettings = {';
  const blocks = [];
  let idx = 0;
  while ((idx = src.indexOf(needle, idx)) !== -1) {
    const start = idx + needle.length - 1; // '{' konumu
    let depth = 0;
    let inQuote = false;
    let i = start;
    for (; i < src.length; i++) {
      const c = src[i];
      if (inQuote) {
        if (c === '\\') i++;
        else if (c === '"') inQuote = false;
        continue;
      }
      if (c === '"') inQuote = true;
      else if (c === '{') depth++;
      else if (c === '}') {
        depth--;
        if (depth === 0) break;
      }
    }
    if (i >= src.length) return null; // dengesiz → dosyaya DOKUNMA
    blocks.push({ start, end: i });
    idx = i;
  }
  return blocks;
}

/** Blok gövdesinde (`{`…`}`) bir build ayarını yazar/günceller. */
function setBuildSetting(body, key, value) {
  const quoted = `"${String(value).replace(/(["\\])/g, '\\$1')}"`;
  // NOT: `[ \t\r]*$` — CRLF satır sonlarında da eşleşsin. Eşleşmezse satır İKİNCİ KEZ
  // eklenir ve pbxproj'da yinelenen anahtar oluşur; bu yüzden \r bilinçli olarak tolere edilir.
  const lineRe = new RegExp(`^([ \\t]*)${key}[ \\t]*=[ \\t]*[^\\n]*;[ \\t\\r]*$`, 'm');
  if (lineRe.test(body)) {
    return body.replace(lineRe, (_m, ind) => `${ind}${key} = ${quoted};`);
  }
  const sample = body.match(/\n([ \t]+)\S/);
  const ind = sample ? sample[1] : '\t\t\t\t';
  const nl = body.indexOf('\n');
  if (nl === -1) {
    // tek satırlık blok: `{ }` → '{' hemen ardına ekle
    return `${body.slice(0, 1)}\n${ind}${key} = ${quoted};${body.slice(1)}`;
  }
  return `${body.slice(0, nl + 1)}${ind}${key} = ${quoted};\n${body.slice(nl + 1)}`;
}

/**
 * Uygulama hedefinin tüm build konfigürasyonlarına şunları yazar:
 *   CODE_SIGN_ENTITLEMENTS   → App/App.entitlements (imzalı IPA'da push + universal link)
 *   MARKETING_VERSION        → kullanıcıya görünen sürüm (CFBundleShortVersionString)
 *   CURRENT_PROJECT_VERSION  → artan build numarası (CFBundleVersion)
 */
function patchPbxproj(app) {
  const pbx = join(
    ROOT, 'capacitor', app.key, 'ios', 'App', 'App.xcodeproj', 'project.pbxproj'
  );
  if (!existsSync(pbx)) die(`project.pbxproj bulunamadı: ${pbx}`);

  const version = rootVersion();
  const build = buildNumber(version);
  const entitlementsPath = join(
    ROOT, 'capacitor', app.key, 'ios', 'App', 'App', 'App.entitlements'
  );

  const settings = [
    ['MARKETING_VERSION', version],
    ['CURRENT_PROJECT_VERSION', build],
  ];
  // Entitlements dosyası gerçekten yerleştiyse bağla (yoksa Xcode "file not found" ile patlar).
  if (existsSync(entitlementsPath)) {
    settings.unshift(['CODE_SIGN_ENTITLEMENTS', 'App/App.entitlements']);
  } else {
    console.log('  ⚠ App.entitlements yok — CODE_SIGN_ENTITLEMENTS bağlanmadı.');
  }

  const original = readFileSync(pbx, 'utf8');
  const blocks = findBuildSettingsBlocks(original);
  if (blocks === null) {
    die(`project.pbxproj süslü parantezleri dengesiz görünüyor — dosyaya dokunulmadı: ${pbx}`);
  }

  let src = original;
  let patched = 0;
  // SONDAN başa: önceki uzunluk değişimleri sonraki ofsetleri bozmasın.
  for (let i = blocks.length - 1; i >= 0; i--) {
    const { start, end } = blocks[i];
    let body = src.slice(start, end + 1);
    if (!body.includes('PRODUCT_BUNDLE_IDENTIFIER')) continue; // proje-düzeyi blok
    for (const [k, v] of settings) body = setBuildSetting(body, k, v);
    src = src.slice(0, start) + body + src.slice(end + 1);
    patched++;
  }

  if (patched === 0) {
    die(
      `project.pbxproj içinde uygulama hedefi build konfigürasyonu bulunamadı ` +
        `(PRODUCT_BUNDLE_IDENTIFIER yok). Capacitor şablonu değişmiş olabilir: ${pbx}`
    );
  }

  // Yazmadan önce son sağlamlık kontrolü.
  if (findBuildSettingsBlocks(src) === null || !src.includes('rootObject')) {
    die(`project.pbxproj yaması doğrulanamadı — dosya YAZILMADI: ${pbx}`);
  }
  if (src !== original) writeFileSync(pbx, src);
  console.log(
    `  ✓ pbxproj (${patched} konfigürasyon): ` +
      `MARKETING_VERSION=${version}, CURRENT_PROJECT_VERSION=${build}` +
      (existsSync(entitlementsPath) ? ', CODE_SIGN_ENTITLEMENTS=App/App.entitlements' : '')
  );
}

// ───────────────────────── iOS ─────────────────────────

function prepareIos(app) {
  const appDir = join(ROOT, 'capacitor', app.key);
  const iosDir = join(appDir, 'ios');
  const overrides = join(appDir, 'ios-overrides');

  if (!existsSync(iosDir)) {
    console.log(`▶ [${app.key}] ios platformu yok — npx cap add ios`);
    sh('npx', ['--no-install', 'cap', 'add', 'ios'], appDir);
  } else {
    console.log(`▶ [${app.key}] ios platformu mevcut — cap add atlanıyor`);
  }
  if (!existsSync(iosDir)) {
    die(`ios/ üretilemedi (${app.key}). @capacitor/ios kurulu mu? (per-app npm install)`);
  }

  const appAppDir = join(iosDir, 'App', 'App');

  // 1) Entitlements dosyasını yerleştir (pbxproj bağlaması 4. adımda).
  copyInto(join(overrides, 'App.entitlements'), appAppDir, 'App.entitlements');

  // 2) Info.plist anahtarlarını birleştir (PlistBuddy).
  mergeInfoPlist(join(overrides, 'Info.plist.partial.xml'), join(appAppDir, 'Info.plist'));

  // 3) AppIcon setini yerleştir (varsa).
  if (ensureResources(app, 'ios')) {
    const iconSrc = join(appDir, 'resources', 'ios', 'AppIcon.appiconset');
    if (existsSync(iconSrc)) {
      const iconDest = join(appAppDir, 'Assets.xcassets', 'AppIcon.appiconset');
      mkdirSync(iconDest, { recursive: true });
      for (const f of readdirSync(iconSrc)) copyInto(join(iconSrc, f), iconDest, f);
    }
  }

  // 4) Xcode projesi: entitlements bağlaması + sürüm alanları.
  patchPbxproj(app);

  console.log(`✓ [${app.key}] ios hazırlandı (${app.url}).`);
}

// ───────────────────────── main ─────────────────────────

function main() {
  const { app: key, platform, step } = parseArgs(process.argv.slice(2));
  if (!key) die('--app <key> gerekli.');
  if (!platform || !['android', 'ios'].includes(platform)) {
    die('--platform <android|ios> gerekli.');
  }
  if (!['all', 'xcodeproj', 'version'].includes(step)) {
    die('--step <all|xcodeproj|version> geçersiz.');
  }
  const app = loadApp(key);

  // www güvenlik ağı (canlı-URL kabuğu için webDir).
  const www = join(ROOT, 'capacitor', app.key, 'www');
  if (!existsSync(www)) mkdirSync(www, { recursive: true });

  console.log(`\n=== prepare: ${app.key} / ${platform} (step: ${step}) ===`);

  // `cap sync` sonrası yeniden uygulanabilen, yalnız-yama adımları.
  if (step === 'xcodeproj' || (step === 'version' && platform === 'ios')) {
    patchPbxproj(app);
  } else if (step === 'version') {
    patchAndroidVersion(app);
  } else if (platform === 'android') {
    prepareAndroid(app);
  } else {
    prepareIos(app);
  }
  console.log('=== prepare tamam ===\n');
}

main();
