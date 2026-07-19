# Ikonlar — CI'da uretilir

Bu klasor **derleme zamaninda** doldurulur. Binary ikonlar repoya konmaz.

Kaynak: `assets/icons/icon-source-dcim.png` (512x512 RGBA).

CI / manuel olarak, `tauri/dcim/` dizininden:

```bash
npx tauri icon ../../assets/icons/icon-source-dcim.png
```

Bu komut asagidakileri **src-tauri/icons/** altina uretir:

- `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`
- `icon.ico` (Windows)
- `icon.icns` (macOS)
- ek Windows Store `Square*Logo.png` / `StoreLogo.png`

`tauri.conf.json > bundle.icon` ve `generate_context!` bu dosyalara ihtiyac duyar;
**`tauri build`'den ONCE bu adim calistirilmalidir**, yoksa derleme "icon not found"
ile basarisiz olur.
