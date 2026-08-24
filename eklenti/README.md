# Bogahost Kasa — Chrome eklentisi

Eklentinin **kendi girişi yoktur.** Kimliğini aynı bilgisayardaki Kasa
uygulamasından alır: uygulama `127.0.0.1` üzerinde küçük bir köprü açıyor
(`tauri/kasa/src-tauri/src/kopru.rs`), eklenti onu buluyor ve sunucuya
uygulama gidiyor.

Önceki sürümde eklentinin kendi API jetonu vardı ve personelin DCIM'e girip
"Eklentiyi Bağla" demesi gerekiyordu. İki ayrı kimliğin karşılığı yoktu:
ikinci bir sır üretiliyor, ikinci bir yerde saklanıyor, hesap kapanınca
ikisinin ayrı ayrı düşmesi gerekiyordu. Artık tek kimlik var.

## Uçlar (yalnız 127.0.0.1)

| Uç | Ne döner |
|---|---|
| `GET /durum` | oturum açık mı, uygulama sürümü |
| `GET /eslesenler?url=…` | o adrese uyan kayıtlar — **parola yok** |
| `POST /doldur` `{id,url}` | kullanıcı adı + parola — yalnız adres kayda uyuyorsa |

İstek `Origin: chrome-extension://<kimlik>` ya da `X-Kasa: <kimlik>`
taşımıyorsa köprü 403 döner. Bir web sayfası bu başlıkların ikisini de
taklit edemez.

## Paketleme

`bogahost-kasa.crx` CRX3 olarak imzalanıp `native.bogahost.com/eklenti/`
altına konuyor; `update.xml` sürümü **manifest sürümüyle aynı olmalı**,
yoksa Chrome paketi yok sayar. İmzalama anahtarı sunucuda
`/home/dcimboga/.anahtarlar/kasa-eklenti.pem` — kimlik ondan türüyor ve
değişirse Chrome bunu bambaşka bir eklenti sayar.
