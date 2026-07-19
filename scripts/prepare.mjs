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
 *        - iOS App.entitlements → yerleştirilir (NOT: pbxproj CODE_SIGN_ENTITLEMENTS
 *          bağlaması imzalı build gerektirir — aşağıdaki UYARI'ya bakın).
 *
 *   assetlinks.json / apple-app-site-association SUNUCU dosyalarıdır (canlı .well-known
 *   altına ELLE konur) — uygulama paketine KOPYALANMAZ; bilinçli olarak atlanır.
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
  const out = { app: null, platform: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--app') out.app = argv[++i];
    else if (a.startsWith('--app=')) out.app = a.split('=')[1];
    else if (a === '--platform') out.platform = argv[++i];
    else if (a.startsWith('--platform=')) out.platform = a.split('=')[1];
  }
  return out;
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

  // 1) Entitlements dosyasını yerleştir.
  //    UYARI: bu yalnızca dosyayı KOYAR. Xcode pbxproj'a CODE_SIGN_ENTITLEMENTS
  //    olarak bağlanması imzalı build (provisioning) gerektirir; imzasız
  //    doğrulama build'inde entitlements zaten uygulanmaz. Bkz. rapor.
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

  console.log(`✓ [${app.key}] ios hazırlandı (${app.url}).`);
}

// ───────────────────────── main ─────────────────────────

function main() {
  const { app: key, platform } = parseArgs(process.argv.slice(2));
  if (!key) die('--app <key> gerekli.');
  if (!platform || !['android', 'ios'].includes(platform)) {
    die('--platform <android|ios> gerekli.');
  }
  const app = loadApp(key);

  // www güvenlik ağı (canlı-URL kabuğu için webDir).
  const www = join(ROOT, 'capacitor', app.key, 'www');
  if (!existsSync(www)) mkdirSync(www, { recursive: true });

  console.log(`\n=== prepare: ${app.key} / ${platform} ===`);
  if (platform === 'android') prepareAndroid(app);
  else prepareIos(app);
  console.log('=== prepare tamam ===\n');
}

main();
