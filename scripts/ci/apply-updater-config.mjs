#!/usr/bin/env node
// Yalnizca CI'da calisir. `tauri.conf.json` icindeki updater PLACEHOLDER'ini
// gercek public key ile degistirir ve updater artifact uretimini acar.
//
//   node scripts/ci/apply-updater-config.mjs --app finans --pubkey "<base64>"
//
// NEDEN BUILD ONCESI ENJEKSIYON?
//   `bundle.createUpdaterArtifacts` acikken Tauri, TAURI_SIGNING_PRIVATE_KEY
//   yoksa BUILD'I HATA ILE SONLANDIRIR. Bu yuzden depoda kapali durur ve yalnizca
//   imzalama secret'i mevcutken CI tarafindan acilir — imzasiz build'ler
//   (bugunku yesil CI) hicbir sekilde etkilenmez.
//
// pubkey verilmezse: hicbir sey degistirilmez, uyari basilir, cikis kodu 0.
// (Uygulama bu durumda calisir; updater eklentisi yuklenmez ve eski manifest
//  denetimine duser — bkz. tauri/*/src-tauri/src/lib.rs)

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const PLACEHOLDER = '__TAURI_UPDATER_PUBKEY__';

function arg(name, fallback = '') {
  const i = process.argv.indexOf(`--${name}`);
  return i !== -1 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

const app = arg('app');
const pubkey = arg('pubkey').trim();

if (!app) {
  console.error('HATA: --app zorunlu (finans|dcim|chat|task)');
  process.exit(1);
}

const configPath = resolve(REPO_ROOT, 'tauri', app, 'src-tauri', 'tauri.conf.json');
const config = JSON.parse(readFileSync(configPath, 'utf8'));

if (!pubkey || pubkey === PLACEHOLDER) {
  console.log(
    `::warning title=Updater kapali::${app} — public key verilmedi. ` +
      'Updater artifact/imza URETILMEYECEK. Anahtar uretimi: docs/UPDATE.md'
  );
  process.exit(0);
}

// Minisign public key, base64 kodlu ~56+ karakterlik tek satirdir.
if (!/^[A-Za-z0-9+/=]{40,}$/.test(pubkey)) {
  console.error(
    'HATA: TAURI_SIGNING_PUBLIC_KEY beklenen bicimde degil (tek satir base64).\n' +
      '.pub dosyasinin ICERIGININ TAMAMI degil, "untrusted comment" satirindan SONRAKI satir kullanilmali.'
  );
  process.exit(1);
}

config.plugins ??= {};
config.plugins.updater ??= {};
config.plugins.updater.pubkey = pubkey;

config.bundle ??= {};
config.bundle.createUpdaterArtifacts = true;

writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`, 'utf8');

console.log(`[updater] ${app}: pubkey enjekte edildi, createUpdaterArtifacts=true`);
console.log(`[updater] ${app}: endpoints = ${JSON.stringify(config.plugins.updater.endpoints)}`);
