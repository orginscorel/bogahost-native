#!/usr/bin/env node
/**
 * Bogahost native — ortam kontrolü (doctor).
 *
 * Toolchain'lerin VAR/YOK durumunu raporlar ve hangi platformun nerede
 * derlenebileceğini özetler. Hiçbir şey kurmaz/derlemez.
 *
 * Kullanım: node scripts/doctor.mjs
 * Node 20+.
 */
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';

const IS_WIN = process.platform === 'win32';

function probe(cmd, args = ['--version']) {
  const res = spawnSync(cmd, args, { encoding: 'utf8', shell: IS_WIN });
  if (res.error || res.status !== 0) return null;
  const out = ((res.stdout || '') + (res.stderr || '')).trim().split('\n')[0];
  return out || 'kurulu';
}

function line(label, value) {
  const mark = value ? '✓' : '✗';
  const status = value ? value : 'YOK';
  console.log(`  ${mark} ${label.padEnd(22)} ${status}`);
}

function main() {
  console.log('Bogahost native — ortam kontrolü\n');

  const node = probe('node');
  const npm = probe('npm');
  const java = probe('java', ['-version']);
  const gradle = probe('gradle', ['-version']) || probe('./gradlew', ['-version']);
  const androidHome = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT || null;
  const androidSdk = androidHome && existsSync(androidHome) ? androidHome : (androidHome ? `${androidHome} (dizin yok)` : null);
  const xcode = process.platform === 'darwin' ? probe('xcodebuild', ['-version']) : null;
  const rustc = probe('rustc');
  const cargo = probe('cargo');

  console.log('Genel:');
  line('Node.js', node);
  line('npm', npm);

  console.log('\nAndroid (Capacitor):');
  line('Java (JDK)', java);
  line('Gradle', gradle);
  line('Android SDK', androidSdk);

  console.log('\niOS (Capacitor, sadece macOS):');
  line('Xcode (xcodebuild)', xcode);

  console.log('\nMasaüstü (Tauri):');
  line('Rust (rustc)', rustc);
  line('Cargo', cargo);

  console.log('\n─── Nerede derlenir ───');
  console.log('  • Android APK/AAB : Java + Gradle + Android SDK olan makine (Linux/macOS/Windows) veya CI.');
  console.log('  • iOS IPA         : Yalnızca macOS + Xcode (veya macOS CI runner).');
  console.log('  • Windows (Tauri) : Windows runner + Rust + MSVC build tools.');
  console.log('  • macOS (Tauri)   : macOS runner + Rust + Xcode CLT.');
  console.log('\n  Not: Bu üretim web sunucusunda toolchain KURULU DEĞİL — derleme GitHub Actions\'ta yapılır.');

  if (process.platform === 'linux' && !java && !rustc) {
    console.log('\n  ℹ Bu makinede beklendiği gibi mobil/masaüstü toolchain yok; kaynaklar CI için hazırdır.');
  }
}

main();
