# Bogahost Kasa (masaüstü)

Bilgi Kasası'ndaki kullanıcı adı ve parolayı **masaüstü programlarına** yazar:
RDP, FileZilla, WinSCP, veritabanı araçları, VPN istemcileri…

Tarayıcılar dahil **her pencereye** yazar: kimlik bilgisi klavye girdisi olarak
gönderildiği için hedefin ne olduğu fark etmez.

## Diğer uygulamalardan farkı

`finans`, `dcim`, `chat`, `task`, `muh` birer **WebView kabuğudur**: canlı bir
URL yüklerler. Kasa bunu yapamaz. İşini görebilmesi için işletim sistemi
yetenekleri gerekir — küresel kısayol, öndeki pencereyi okuma, başka bir
programa tuş gönderme. Bu yüzden:

- arayüz **yereldir** (`dist/`), uzak URL yüklenmez,
- iş mantığı **Rust tarafındadır**,
- `apps.config.json` kaydında `url` alanı yoktur (`yerel: true`).

## Güvenlik tasarımı

| Karar | Neden |
|---|---|
| Parola arayüze **hiç verilmez** | Arayüzde bir açık olsa bile kasadan parola sızdırılamaz. Parola sunucudan alınır, doğrudan klavyeye yazılır, işlev biterken bellekten düşer. |
| Jeton **işletim sistemi kasasında** | Windows Credential Manager / macOS Keychain. Kendi şifrelememizi yazmak yerine anahtar yönetimi, kullanıcı izolasyonu ve kilit ekranı koruması hazır gelir. |
| `capabilities/default.json` **en az yetki** | Arayüz ağa çıkamaz, dosya sistemine erişemez. Ağ ve dosya işi yalnızca Rust komutlarından geçer. |
| Girişte **2FA zorunlu** | Sunucu (`/vault/api/giris`) 2FA kurulu olmayan hesaba jeton vermez. |
| Her kullanıcı **kendi kayıtlarını** görür | Ortak havuz yok; `/vault/api/items` ve `/reveal` jetonun sahibine göre kapsamlanır. |
| Panoya kopyalama **30 sn sonra temizlenir** | Araya başka bir şey kopyalandıysa panoya dokunulmaz. |

## Hedef nasıl seçilir

İki yol var:

1. **Listeden** — uygulama açık programları tarar, üstteki açılır listeden
   seçersiniz. "Doldur" dendiğinde o pencere önce öne getirilir.
2. **Kısayolla** — hedef pencere öndeyken kısayola basarsınız, o anki pencere
   yakalanır.

Başta yalnızca kısayol vardı; uygulama doğrudan açıldığında ya da macOS
Otomasyon izni verilmemişken ekranda seçilecek hiçbir şey kalmıyordu.

## Kısayol

- Windows: `Ctrl + Alt + K`
- macOS: `Cmd + Alt + K`

Basıldığı anda **önce hedef pencere yakalanır**, sonra Kasa penceresi açılır.
Sıra önemlidir: kendimizi önce gösterirsek öndeki pencere biz oluruz.

## macOS izinleri

İlk kullanımda iki ayrı izin sorulur — ikisi de gereklidir:

1. **Otomasyon (Apple Events)** — öndeki pencerenin adını okumak için.
2. **Erişilebilirlik** — başka programa tuş göndermek için.
   *Sistem Ayarları → Gizlilik ve Güvenlik → Erişilebilirlik → Bogahost Kasa*

İkincisi verilmezse yazma sessizce başarısız olur; uygulama doldurmadan önce
bunu sınar ve kullanıcıyı yönlendirir.

## Derleme

CI (`.github/workflows/windows.yml`, `macos.yml`) `kasa`yı da matriste derler.
Yerelde:

```bash
npm install
npm run tauri:build:win   # MSI + NSIS
npm run tauri:build:mac   # DMG + .app
```

## Gizli kayıtlar — "doldurabilsin ama görmesin"

Bir kayıt panelde **Gizli** işaretlenirse, paylaşıldığı kişide:

- şifre ve kullanıcı adı **ekranda gösterilmez**,
- **panoya kopyalanamaz** (düğmeler kapalı, sunucu da reddeder),
- yalnızca hedef pencereye **yazılabilir**, ve her yazım denetime düşer.

Kaydın **sahibi** her zaman görür — kendi bilgisini göremeyen bir tasarım işe yaramaz.

Teknik ayrım: görüntüleme `/vault/api/reveal` ucundan, doldurma `/vault/api/fill`
ucundan geçer. Gizli kayıtta ilki 403 döner, ikincisi çalışır.

### Dürüst sınır

Bilgiyi hedefe **yazabilen** bir istemci, o bilgiyi **elde etmiştir**. Bu tasarım
kimlik bilgisinin uygulama ekranında görünmemesini ve panoya düşmemesini garanti
eder; uygulamayı tersine çeviren birine karşı koruma sağlamaz. Asıl caydırıcı,
her kullanımın kim/ne/ne zaman olarak iz bırakmasıdır.

Bir de şu var: parola hedef programa yazıldıktan sonra o programın denetimindedir.
Tarayıcıda bir parola alanı geliştirici araçlarıyla düz metne çevrilebilir. Bunu
uygulama tarafından engellemek mümkün değil.

## Tarayıcının parolayı KENDİ kasasına kaydetmesini engelleme

Doldurma bittiğinde tarayıcı "Şifreyi kaydedeyim mi?" diye sorabilir; kabul
edilirse parola tarayıcının kasasına, oradan da kullanıcının hesabına ve tüm
cihazlarına eşitlenir. **Bunu uygulama engelleyemez** — karar tarayıcıya aittir.

Engellemenin desteklenen tek yolu **yönetilen politikadır**. `politika/` altında
hazır dosyalar var:

| Dosya | Ne yapar |
|---|---|
| `politika/macos-chrome-edge.sh` | `sudo bash ...` — Chrome/Edge/Brave parola kasasını kapatır (`/Library/Managed Preferences`). Kullanıcı ayarlardan geri açamaz. |
| `politika/windows-chrome-edge.reg` | Yönetici olarak birleştirin — aynı politikayı `HKLM\SOFTWARE\Policies` altına yazar. |

Uygulandıktan sonra tarayıcıyı tamamen kapatıp açın; `chrome://policy` adresinde
`PasswordManagerEnabled = false` görünmeli.

**Ayrıca:** Kasa varsayılan olarak doldurma sonrası **Enter'a basmaz**. Kaydetme
balonunu tetikleyen şey formun gönderilmesidir; göndermeyi kullanıcıya bırakmak
balonun çıkma olasılığını da azaltır.
