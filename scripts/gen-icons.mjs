#!/usr/bin/env node
/**
 * Bogahost native — ikon & açılış (splash) üreteci.
 *
 * Her uygulamanın iconSource'undan (apps.config.json) şunları üretir:
 *   capacitor/<key>/resources/
 *     icon.png                         (1024, kanonik)
 *     icon-foreground.png              (1024, güvenli alan paddingli — adaptive ön plan)
 *     icon-background.png              (1024, düz backgroundColor)
 *     splash.png / splash-dark.png     (2732x2732, ortalanmış logo)
 *     android/mipmap-<dpi>/            ic_launcher / _round / _foreground (mdpi→xxxhdpi)
 *     ios/AppIcon.appiconset/          tüm iOS boyutları + Contents.json
 *
 * `sharp` gerektirir. Yoksa NET uyarı basılır ve çıkış 0 olur (çökmez).
 *
 * Kullanım: node scripts/gen-icons.mjs [--app <key|all>]
 * Node 20+.
 */
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');

// Android yoğunlukları
const LAUNCHER = { mdpi: 48, hdpi: 72, xhdpi: 96, xxhdpi: 144, xxxhdpi: 192 };
const FOREGROUND = { mdpi: 108, hdpi: 162, xhdpi: 216, xxhdpi: 324, xxxhdpi: 432 };

// iOS AppIcon boyutları: [pikselBoyutu, dosyaAdı, idiom, "PtxScale"]
const IOS_ICONS = [
  [40, 'icon-20@2x.png', 'iphone', '20x20', '2x'],
  [60, 'icon-20@3x.png', 'iphone', '20x20', '3x'],
  [58, 'icon-29@2x.png', 'iphone', '29x29', '2x'],
  [87, 'icon-29@3x.png', 'iphone', '29x29', '3x'],
  [80, 'icon-40@2x.png', 'iphone', '40x40', '2x'],
  [120, 'icon-40@3x.png', 'iphone', '40x40', '3x'],
  [120, 'icon-60@2x.png', 'iphone', '60x60', '2x'],
  [180, 'icon-60@3x.png', 'iphone', '60x60', '3x'],
  [20, 'icon-20.png', 'ipad', '20x20', '1x'],
  [40, 'icon-20@2x-ipad.png', 'ipad', '20x20', '2x'],
  [29, 'icon-29.png', 'ipad', '29x29', '1x'],
  [58, 'icon-29@2x-ipad.png', 'ipad', '29x29', '2x'],
  [40, 'icon-40.png', 'ipad', '40x40', '1x'],
  [80, 'icon-40@2x-ipad.png', 'ipad', '40x40', '2x'],
  [76, 'icon-76.png', 'ipad', '76x76', '1x'],
  [152, 'icon-76@2x.png', 'ipad', '76x76', '2x'],
  [167, 'icon-83.5@2x.png', 'ipad', '83.5x83.5', '2x'],
  [1024, 'icon-1024.png', 'ios-marketing', '1024x1024', '1x'],
];

function loadApps() {
  const cfg = JSON.parse(readFileSync(join(ROOT, 'apps.config.json'), 'utf8'));
  return cfg.apps || [];
}

function parseApp(argv) {
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--app') return argv[i + 1];
    if (argv[i].startsWith('--app=')) return argv[i].split('=')[1];
  }
  return 'all';
}

function iosContentsJson() {
  const images = IOS_ICONS.map(([, file, idiom, size, scale]) => ({
    idiom,
    size,
    scale,
    filename: file,
  }));
  return JSON.stringify({ images, info: { version: 1, author: 'bogahost-native' } }, null, 2);
}

async function generateForApp(sharp, app) {
  const srcPath = join(ROOT, app.iconSource);
  const resDir = join(ROOT, 'capacitor', app.key, 'resources');
  const bg = app.backgroundColor || '#0e1015';
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };

  mkdirSync(resDir, { recursive: true });

  const srcBuf = readFileSync(srcPath);

  // ── Kanonik kaynaklar ──
  await sharp(srcBuf).resize(1024, 1024, { fit: 'contain', background: transparent }).png().toFile(join(resDir, 'icon.png'));

  // Ön plan: 1024 tuval, logo %62 güvenli alanda ortalı.
  await makeForeground(sharp, srcBuf, 1024, transparent, join(resDir, 'icon-foreground.png'));

  // Arka plan: düz renk 1024.
  await sharp({ create: { width: 1024, height: 1024, channels: 4, background: bg } })
    .png().toFile(join(resDir, 'icon-background.png'));

  // Splash 2732: bg üstünde ortalanmış logo (~%38).
  await makeSplash(sharp, srcBuf, 2732, bg, join(resDir, 'splash.png'));
  await makeSplash(sharp, srcBuf, 2732, bg, join(resDir, 'splash-dark.png'));

  // ── Android mipmap'leri ──
  for (const [dpi, size] of Object.entries(LAUNCHER)) {
    const dir = join(resDir, 'android', `mipmap-${dpi}`);
    mkdirSync(dir, { recursive: true });
    // Legacy ikon: bg üstüne düzleştir (şeffaflık kenarı olmasın).
    await sharp(srcBuf).resize(size, size, { fit: 'contain', background: transparent })
      .flatten({ background: bg }).png().toFile(join(dir, 'ic_launcher.png'));
    await sharp(srcBuf).resize(size, size, { fit: 'contain', background: transparent })
      .flatten({ background: bg }).png().toFile(join(dir, 'ic_launcher_round.png'));
  }
  for (const [dpi, size] of Object.entries(FOREGROUND)) {
    const dir = join(resDir, 'android', `mipmap-${dpi}`);
    mkdirSync(dir, { recursive: true });
    await makeForeground(sharp, srcBuf, size, transparent, join(dir, 'ic_launcher_foreground.png'));
  }

  // ── iOS AppIcon seti ──
  const iconSet = join(resDir, 'ios', 'AppIcon.appiconset');
  mkdirSync(iconSet, { recursive: true });
  for (const [size, file] of IOS_ICONS) {
    // App Store alpha kabul etmez → bg üstüne düzleştir.
    await sharp(srcBuf).resize(size, size, { fit: 'contain', background: transparent })
      .flatten({ background: bg }).png().toFile(join(iconSet, file));
  }
  writeFileSync(join(iconSet, 'Contents.json'), iosContentsJson());

  console.log(`✓ [${app.key}] ikon & splash üretildi → capacitor/${app.key}/resources/`);
}

async function makeForeground(sharp, srcBuf, size, transparent, dest) {
  const inner = Math.round(size * 0.62);
  const logo = await sharp(srcBuf).resize(inner, inner, { fit: 'contain', background: transparent }).png().toBuffer();
  await sharp({ create: { width: size, height: size, channels: 4, background: transparent } })
    .composite([{ input: logo, gravity: 'center' }])
    .png().toFile(dest);
}

async function makeSplash(sharp, srcBuf, size, bg, dest) {
  const inner = Math.round(size * 0.38);
  const logo = await sharp(srcBuf).resize(inner, inner, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  await sharp({ create: { width: size, height: size, channels: 4, background: bg } })
    .composite([{ input: logo, gravity: 'center' }])
    .png().toFile(dest);
}

async function main() {
  let sharp;
  try {
    ({ default: sharp } = await import('sharp'));
  } catch {
    console.error('✗ "sharp" bulunamadı. İkon üretmek için: npm install (devDependency olarak tanımlı).');
    console.error('  CI, ikon adımından önce bağımlılıkları kurar. Şimdilik atlanıyor.');
    process.exit(0);
  }

  const apps = loadApps();
  const key = parseApp(process.argv.slice(2));
  const targets = key === 'all' ? apps : apps.filter((a) => a.key === key);
  if (targets.length === 0) {
    console.error(`✗ Bilinmeyen uygulama: "${key}". Geçerli: ${apps.map((a) => a.key).join(', ')}`);
    process.exit(2);
  }

  for (const app of targets) {
    try {
      await generateForApp(sharp, app);
    } catch (err) {
      console.error(`✗ [${app.key}] üretim hatası: ${err.message}`);
      process.exitCode = 1;
    }
  }
}

main();
