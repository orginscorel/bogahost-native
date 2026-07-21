# Yayınlama / Dağıtım (PUBLISH)

Ön koşul: **imzalı** artifact'lar. Bkz. [SIGNING.md](SIGNING.md). CI artifact'ları
Actions çalıştırmasının **Artifacts** bölümünden veya `release.yml` ile oluşan **GitHub Release**'ten
indirilir.

5 uygulama = 5 ayrı mağaza kaydı (`com.bogahost.finans/.dcim/.chat/.task/.muh`). Bunlar **iç yönetim
araçlarıdır**; herkese açık mağaza yerine sınırlı/dahili dağıtım (aşağıda) çoğu zaman daha uygundur.

---

## Google Play (Android — AAB)

1. [Google Play Console](https://play.google.com/console) hesabı (**tek seferlik 25 USD**).
2. Her uygulama için yeni uygulama oluştur (paket adı = `com.bogahost.<key>`).
3. **Play App Signing**'i etkinleştir (önerilir).
4. **AAB** dosyasını (`bundleRelease` çıktısı) bir sürüm parkuruna yükle:
   - **Internal testing** → en hızlı, dahili ekip için ideal (link ile 100 test kullanıcıya kadar).
   - **Closed / Open testing** → daha geniş.
   - **Production** → herkese açık.
5. Store listing, gizlilik politikası, içerik derecelendirmesi doldur → incelemeye gönder.

> **Dahili öneri:** Mağaza incelemesi istemiyorsanız AAB yerine **APK**'yı MDM veya doğrudan link
> ile dağıtın (aşağıdaki "Doğrudan dağıtım").

## App Store / TestFlight (iOS — IPA)

1. [App Store Connect](https://appstoreconnect.apple.com) → her `com.bogahost.<key>` için uygulama kaydı.
2. İmzalı **IPA**'yı yükle:
   ```bash
   xcrun altool --upload-app -f finans.ipa -t ios \
     -u "$APPLE_ID" -p "$APPLE_APP_SPECIFIC_PASSWORD"
   # veya Transporter.app ile sürükle-bırak
   ```
3. **TestFlight** → dahili test (App Store Connect'teki 100 iç kullanıcı, inceleme yok) veya
   harici test (hafif inceleme). Dahili araçlar için TestFlight çoğunlukla yeterlidir.
4. Herkese açık yayın için App Store incelemesine gönder.

> iOS'ta mağaza dışı dağıtımın tek meşru yolu **Apple Business Manager + custom apps** veya
> **Ad Hoc** (cihaz UDID'leri kayıtlı, 100 cihaz sınırı). Serbest yan-yükleme yoktur.

## Microsoft Store / doğrudan MSI (Windows)

- **Doğrudan dağıtım (önerilen, iç araç):** İmzalı **MSI** veya **NSIS EXE**'yi intranet/indirme
  linkiyle paylaşın. Authenticode imzalı ise SmartScreen uyarısı çıkmaz.
- **Microsoft Store:** [Partner Center](https://partner.microsoft.com) hesabı → uygulama gönder →
  MSI/MSIX paketi yükle → inceleme. Tauri MSI'ı doğrudan kabul edilir; MSIX gerekirse ek paketleme
  gerekir.

## macOS dağıtım (DMG)

1. Artifact: **notarize edilmiş** `.dmg` (`macos.yml`, imza secret'ları tanımlıysa notarize eder).
2. Notarization "stapling"i doğrula:
   ```bash
   xcrun stapler validate Bogahost-Finans.dmg
   spctl -a -vv -t install Bogahost-Finans.app   # "accepted, notarized" beklenir
   ```
3. DMG'yi indirme linkiyle dağıtın. Notarize edilmemiş DMG Gatekeeper tarafından "doğrulanamadı"
   uyarısı alır — dahili kullanımda sağ tık → Aç ile açılabilir ama önerilmez.
4. Mac App Store isterseniz ayrı bir "Mac App Store" provisioning + App Store Connect kaydı gerekir
   (Developer ID dağıtımından farklı imza).

---

## Doğrudan dağıtım (mağazasız, dahili ekip)

Bu 5 sistem iç araç olduğundan en pratik yol genellikle mağaza değil:

| Platform | Dosya | Yöntem |
|----------|-------|--------|
| Android | imzalı APK | İndirme linki / MDM (kullanıcı "bilinmeyen kaynak" izni verir) |
| iOS | — | TestFlight veya Ad Hoc (UDID) — serbest yan-yükleme yok |
| Windows | imzalı MSI/EXE | İndirme linki / GPO / intranet |
| macOS | notarize DMG | İndirme linki |
| Tümü | — | **PWA olarak kur** — bkz. [PWA.md](PWA.md) (mağaza/imza gerektirmez) |

> Çoğu iç kullanım senaryosu için en düşük sürtünmeli çözüm **PWA kurulumudur**: mağaza kaydı,
> imzalama ücreti ve inceleme yok. Native binary'ler yalnızca mağaza varlığı veya derin OS
> entegrasyonu gerektiğinde.
