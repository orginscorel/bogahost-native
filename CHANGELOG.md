## 1.26.1

### Düzeltildi
- **Panel artık yalnızca önerecek bir şey varken çıkıyor.** Üç ayrı yol onu
  boşuna açıyordu:
  1. Pencere değiştikçe kendiliğinden tazelenen panel, eşleşme bulamayınca
     ekranda kalıp *"Bu pencereye uyan kayıt yok"* yazıyordu. Bir şey
     önermeyecekse görünmesinin sebebi yok — kapanıyor.
  2. Masaüstü programlarında eşleşme çıkmayınca kasadaki ilk on iki kayıt
     listeleniyordu. Bu liste artık YALNIZ kullanıcı paneli kendisi açtığında
     gösteriliyor; kendiliğinden açılışta yardım değil rahatsızlıktı.
  3. Elle kapatılan panel bir buçuk saniye sonra geri geliyordu, çünkü eşleşme
     hâlâ duruyordu. Kapatmak artık bir cevap: *"şimdi değil"*. O hedefte panel
     kendiliğinden açılmıyor; başka bir pencereye geçince kayıt temizleniyor ve
     kısayol her zaman çalışmaya devam ediyor.

## 1.26.0

### Eklendi
- **Masaüstü programlarında otomatik eşleşme.** Kasa şimdiye kadar yalnız
  tarayıcıyı tanıyordu: adres okunamayan her hedefte panel kapanıyor,
  "eşleştirecek bir şey yok" deniyordu. WinBox açıkken MikroTik kayıtları
  (yalnızca IP taşıyorlar) hiç önerilmedi; panel masaüstü programlarında
  kendiliğinden hiç açılmadı. Artık ölçüt programın adı ve pencere başlığı.
- **Bağ kendiliğinden öğreniliyor.** Bir kaydı bir programa elle doldurduğunuzda
  o programın adı kayda yazılıyor; bir dahaki sefere kayıt kendiliğinden
  öneriliyor. Her program için önceden ayar yapmayı beklemek, özelliğin hiç
  kullanılmaması demekti. Yanlış öğrenilen desen panelden silinebilir.
- Kayıt formunda **Masaüstü programları** alanı (virgülle ayrılmış desenler).

### Düzeltildi
- Panel masaüstü hedefinde eşleştirme yapmıyor, kayıtların ilk on ikisini
  sıralıyordu — doğru kaydı bulmak yine kullanıcının işiydi.
- Doldurma sonrası susturma anahtarı masaüstü programlarında **hepsini birden**
  susturacaktı: `alan_adi` iki nokta üstünde kestiği için tüm programlar aynı
  anahtara düşüyordu. Anahtar artık programa özgü.

## 1.25.3

### Düzeltildi
- **Adresinde yol taşıyan kayıtlar kendi giriş sayfalarında hiç çıkmıyordu.**
  Yol denetimi TAM EŞİTLİK istiyordu; Grafana kaydı bir pano bağlantısı
  (`/d/netops-v2/...`) olduğu için `/login` sayfasında, WHMCS yönetim kaydı
  `/smeownerbogap` olduğu için `/smeownerbogap/login.php` sayfasında asla
  önerilmedi. Yol kısıtı artık bir tahmin değil, kaydın beyanı: yalnız
  `eslesme = "yol"` diyen kayıtlar yolu tutturmak zorunda ve bu tutturma
  **segment sınırında** yapılıyor (`/smeownerbogap2` asla `/smeownerbogap`
  kaydını yakalamaz).
- **Ters alt alan tahmini doldurmaya sunulmuyor.** Ana alanda dururken o alanın
  altındaki her panel öneriliyordu; `bogahost.com` ana sayfasında vCenter,
  Plesk ve Observium kayıtları çıkıyordu.

### Eklendi
- Kayıt bazında **otomatik doldurma eşleşme kuralı** (`alan` · `host` · `yol` ·
  `kapali`). Ana alanı hem halka açık site hem yönetim paneli olan kurumlarda
  şart: aksi halde yönetici parolası o alanın her alt adresinde doldurulmaya
  sunuluyor. Kural DCIM panelindeki kayıt formundan seçilir.
- Eşleşme kararının **tek sahibi sunucu**. İstemci ikinci bir kural işletmiyor;
  iki tarafın ayrı kural hesaplaması sessiz açık üreten şeydi.

## 1.6.0

Sürüm bilgisi artık **her yerde** görünür. Önceki sürümlerde sürüm rozeti yalnızca
giriş ekranında çiziliyordu; kullanıcı giriş yaptıktan sonra uygulamanın hangi
sürüm olduğunu hiçbir yerden göremiyordu.

### Eklendi
- **Tepsi (tray) menüsünde sürüm satırı.** Menünün en üstünde, pasif (tıklanamaz)
  bilgi öğesi olarak `Bogahost <Uygulama> v<sürüm>` gösterilir. Oturum durumundan
  ve sayfadan bağımsızdır — en garantili görünürlük yolu.
- **macOS menü çubuğunda sürüm.** Uygulama menüsünün en üstüne pasif
  `Sürüm v<x.y.z>` öğesi eklendi.
- **"Hakkında" diyaloğu.** Hem tepsi menüsünden ("Hakkında…") hem macOS uygulama
  menüsünden ("Hakkında ve Güncelleme…") açılır; uygulama adı, sürüm ve
  "Powered by Bogahost" bilgisini gösterir, üzerinde **Güncellemeleri denetle**
  düğmesi vardır (tepsi menüsündeki denetimin aynısını çalıştırır, aynı tekillik
  koruması geçerlidir).

### Değişti
- `window.__BOGAHOST_NATIVE_VERSION__` enjeksiyonu korundu (canlı paneller
  sidebar'daki "Uygulama v…" satırı için bunu okur). Asıl enjeksiyon
  `initialization_script` ile **her gezinmede, sayfanın kendi script'lerinden önce**
  (document-start) yapılır; ek olarak sayfa yüklemesi bittiğinde bir **emniyet ağı**
  tazelemesi eklendi.
- Giriş ekranındaki mevcut sürüm rozeti **olduğu gibi bırakıldı**.

### Not
- Dört uygulamada da `lib.rs` birebir aynıdır; yalnızca `APP_KEY` / `APP_TITLE`
  sabitleri ve başlık yorumu farklıdır.
- Canlı panellerin (Finans / DCIM / Chat / Görevler) **Sürüm Notları** sayfasına,
  panel girdilerinden görsel olarak ayrı bir **"Masaüstü Uygulaması (native)"**
  bölümü eklendi; bu liste tek ortak kaynaktan (`config/native_changelog.php`)
  üretilir.

### Düzeltildi — sunucu bildirimleri masaüstüne HİÇ gelmiyordu (kritik)
- **Kök neden bulundu.** Paneller masaüstü bildirimini **service worker + web push**
  ile gösteriyor. Native WebView'de `serviceWorker` ve `PushManager` **yoktur**;
  panelin `pushInit()` fonksiyonu ilk satırda sessizce çıkıyor, bildirim yalnızca
  sayfa içi baloncuk + bip olarak kalıyordu. Panel `new Notification()` **hiç
  çağırmadığı** için 1.5.0'daki Notification köprüsü de asla tetiklenmiyordu.
  Sonuç: DCIM / Görevler / Finans / Chat — dördünde de masaüstünde hiçbir bildirim yok.
- **Çözüm.** Kabuk artık panelin **kendi** bildirim beslemesi yanıtını dinliyor
  (`fetch` sarmalayıcı). Panel zaten ~30 sn'de bir yokladığı için **sunucuya ek yük
  binmez** — bugün yaşanan 429 sorunu tekrarlanmaz. Yeni kayıt geldiğinde native
  masaüstü bildirimi gösterilir.
- Tekilleştirme `localStorage` imleci ile yapılır: aynı bildirim iki kez çıkmaz,
  ilk açılışta **geçmiş bildirimler toplu gösterilmez** (yalnız imleç kurulur).
  Panelin kendi `new Notification()` çağrısı da aynı tekilleştirmeden geçer.
- Panel yoklaması hiç görülmezse 45 sn sonra **yedek yoklama** devreye girer
  (30 sn taban, 429/hata durumunda üstel geri çekilme 5 dk'ya kadar, 401/403'te durur).
- Bildirimin hedef adresi saklanır; tepsi menüsündeki **"Son bildirimi aç"** öğesi
  ilgili sayfayı ana pencerede açar. (Masaüstünde bildirimin kendisine tıklama
  olayı `tauri-plugin-notification` tarafından **sunulmuyor**.)
- **Sınır:** bu yol yalnızca **uygulama açıkken** çalışır. Uygulama kapalıyken
  bildirim gelmesi gerçek APNs push gerektirir — bkz. `docs/PUSH.md`.

### Düzeltildi — kamera / mikrofon (sesli & görüntülü arama)
- **macOS kök nedeni:** `Info.plist`'te `NSCameraUsageDescription` /
  `NSMicrophoneUsageDescription` **yoktu**. Bu anahtarlar olmadan macOS TCC katmanı
  kamera/mikrofona erişmeye çalışan uygulamayı **izin sorusu bile göstermeden
  öldürür** — "hiç tepki vermiyor" şikâyetinin sebebi buydu. Ayrıca Tauri'de
  `hardenedRuntime` **varsayılan olarak açıktır** ve entitlement verilmeden
  kamera/mikrofon çekirdek tarafından reddedilir.
- `src-tauri/Info.plist` (Türkçe açıklama metinleriyle) ve
  `src-tauri/Bogahost.entitlements` eklendi; entitlements dosyası
  `bundle.macOS.entitlements` ile bağlandı.
- İzin reddedilirse artık **sessiz kalınmıyor**: ne olduğunu anlatan bir kutu ve
  ilgili sistem ayarını açan düğme gösteriliyor (macOS ve Windows).

### Düzeltildi — sürükle-bırak ile dosya yükleme
- Tauri'nin kendi sürükle-bırak işleyicisi varsayılan olarak açıktı ve işletim
  sistemi olayını yutuyordu; bunun yan etkisi sayfanın `drop` olayının **hiç
  tetiklenmemesi**, yani `<input type=file>` alanına dosya sürüklenememesiydi.
  Bu davranış Windows'a özgü değildir, macOS'ta da geçerlidir.
  `disable_drag_drop_handler()` ile kapatıldı (ana pencere + önizleme penceresi).

### Düzeltildi — bildirim sesi / arama zili (Windows)
- WebView2'nin otomatik oynatma yasağı zili susturuyordu.
  `--autoplay-policy=no-user-gesture-required` eklendi; Tauri'nin varsayılan
  argümanları **yerine geçtiği** için varsayılanlar aynen korundu.
- macOS'ta karşılığı yoktur (wry'de var, Tauri dışarı açmıyor); orada ses kilidi
  ilk kullanıcı hareketinde açılır.

### Değişti — dürüstlük düzeltmeleri
- Tepsideki "Bildirimler: açık" göstergesi **Windows'ta her zaman "açık"
  diyordu** — çünkü `tauri-plugin-notification` orada izin kavramı olmadığı için
  daima `Granted` döner. Artık Windows'ta durum iddia edilmiyor,
  "Windows ayarlarından yönetilir" denip ayara yönlendiriliyor.
- Pano kopyalama başarısız olursa `execCommand` yedeğine düşülüyor.
- macOS'ta HTML tam ekran (`element.requestFullscreen`) çalışmadığı için
  başarısızlıkta **pencere** tam ekran yapılıyor.

### Bilinen sınırlar (kabuk tarafından aşılamaz)
- **macOS'ta 4 uygulama oturum/çerez paylaşmaz.** `data_directory` yalnızca
  Windows ve Linux arka uçlarında etkilidir; WKWebView'de wry bu değeri sessizce
  yok sayar. Her kabukta ayrı giriş yapılır.
- **Geolocation** masaüstünde çalışmaz (wry ilgili temsilciyi bağlamıyor).
- **Ekran paylaşımı** macOS 14.0–14.5 aralığında bozuk olabilir (wry#1195, hâlâ açık).
- `tauri.conf.json` içindeki **CSP uzak panellere uygulanmaz** — Tauri onu yalnızca
  kendi sunduğu yerel içeriğe HTTP başlığı olarak ekler. Panellerin CSP'si
  sunucudan gelir.

## 1.5.1

### Düzeltildi
- **Uygulamalar arası geçişte yükleme göstergesi görünmüyordu.** Gösterge sayfanın
  DOM'una ekleniyordu; hemen ardından gelen `navigate()` sayfayı yıktığı için gösterge
  de onunla siliniyor, kullanıcı geçişin başladığını anlamıyordu. Artık navigasyondan
  etkilenmeyen **native pencere** (splash) gösteriliyor ve hedef sayfa yüklenince kapanıyor.

## 1.5.0

Kullanıcı geri bildirimi üzerine **masaüstü (Tauri) kabuklarına** odaklanan sürüm.
Dört uygulamada da (Finans / DCIM / Chat / Görevler) `lib.rs` birebir aynıdır;
yalnızca `APP_KEY` / `APP_TITLE` sabitleri farklıdır.

### Düzeltildi — "Bu tarayıcı bildirimi desteklemiyor"
- **Kök neden:** Tauri WebView'i `window.Notification` **sunmuyor**. Panellerin
  Bildirim/Cihazlar ekranı bu nesneyi arayıp bulamayınca "desteklemiyor" diyordu.
- Sayfaya enjekte edilen betiğe **Notification API köprüsü** eklendi: standart
  `new Notification(...)`, `Notification.permission` ve `Notification.requestPermission()`
  artık native masaüstü bildirimine bağlanıyor (`bogahost_notify`,
  `bogahost_notify_state`, `bogahost_notify_request` komutları).
- `requestPermission()` sistem izin penceresini **arka planda** açar (arayüz donmaz),
  sonucu kısa aralıklarla yoklar. İzin verilince kullanıcının görmesi için bir
  **test bildirimi** gösterilir.
- İzin henüz alınmamışken durum `"denied"` değil **`"default"`** raporlanır; böylece
  panel "Bildirim aç" düğmesini gizlemez.
- **Sınır (dürüst not):** Bu **gerçek web-push değildir.** WebView'de `PushManager`
  yoktur; uygulama kapalıyken sunucudan bildirim gelmez. Panel açıkken üretilen
  bildirimler masaüstünde görünür. Ayrıntı: `docs/PUSH.md`.

### Düzeltildi — PDF/CSV indirmede 404
- **Kök neden:** `target="_blank"` taşıyan indirme linkleri ve `window.open(...)`
  çağrıları **ana pencereyi** indirme adresine götürüyordu (`location.href`).
  Sunucu eki (attachment) yerine hata/yönlendirme dönerse panelin kendisi 404
  sayfasına düşüyordu. `target="_blank"` taşıyan **POST form'ları** ise WebView'de
  hiç çalışmıyordu (yeni pencere açılamaz).
- Artık panellerin **üç indirme biçimi de** kapsanıyor:
  1. `GET` + `Content-Disposition` → WebView'in kendi indirme akışı (değişmedi),
  2. `download` niteliği / yeni sekme hedefli indirme linkleri → oturum çerezleriyle
     `fetch` edilip `bogahost_save_file` köprüsüyle diske yazılır, **panel yerinde kalır**,
  3. `target="_blank"` **POST form'ları** → form verisiyle istek atılır, sonuç diske yazılır.
- Dosya adı `Content-Disposition` başlığından okunur (`filename*` UTF-8 dahil).
- **Sessiz 404 kalktı:** hata durumunda sayfada anlaşılır bir mesaj + native bildirim
  gösterilir ("Dosya bulunamadı (404)…", "Bu dosyayı indirme izniniz yok." vb.).
- Çok büyük dosyalarda (>48 MB) köprü yerine WebView'in kendi indirme akışına düşülür.

### Düzeltildi — Açılan PDF/CSV'den uygulamaya geri dönülemiyordu
- **Kök neden:** Yeni sekmede açılmak istenen iç adresler ana pencerede açılıyordu.
  Sunucu PDF/görsel döndürünce WebView dosyayı yerinde görüntülüyor, panel kayboluyor
  ve görünür bir "geri" yolu kalmıyordu — kullanıcı kilitleniyordu.
- Bu adresler artık **ayrı, çerçeveli ve kapatılabilir bir önizleme penceresinde**
  açılıyor (`popup-*`): başlık çubuğu + kapat düğmesi, **ESC** ile kapanma, sağ üstte
  belirgin **"Kapat (ESC)"** ve **"Yazdır"** düğmeleri. **Ana pencere panelde kalır.**
- Ek kurtarma yolu: Görünüm menüsüne **"Panele dön" (Cmd/Ctrl+Shift+H)** eklendi —
  pencere herhangi bir sebeple panelden koptuysa tek tıkla geri döner.
- İndirilen dosya sistem varsayılan uygulamasında açılmaya devam eder; ana pencere
  ele geçirilmez.

### Düzeltildi — Uygulamalar arası geçişte tekrar giriş isteniyordu
- **Kök neden:** Bu projede **SSO bilinçli olarak yoktur**; her uygulama kendi alan
  adında ayrı doğrular. Ancak 4 kabuğun **her biri kendi WebView çerez deposunu**
  kullanıyordu: DCIM uygulamasında alınan oturum çerezi, Finans uygulamasından DCIM'e
  geçildiğinde **görünmüyordu**.
- 4 kabuk artık **ortak bir WebView veri klasörünü** paylaşıyor
  (`<local-data>/BogahostNative/webview`). Her uygulamaya **bir kez** giriş yapılır;
  geçişlerde ve uygulama yeniden açıldığında oturum korunur.
- Bu **SSO değildir** — sunucu tarafına dokunulmadı, yalnızca çerezler paylaşılıyor.
- **Not:** Bu sürüme geçerken çerez deposu değiştiği için her uygulamada **bir kereye
  mahsus** yeniden giriş gerekir.

### Düzeltildi — Task → Finans geçişi çalışmıyor görünüyordu
- 4 uygulamanın `APPS` tablosu, menü kurulumu, CSP host listesi ve Capacitor
  `allowNavigation` listesi karşılaştırıldı: **hepsi simetrik**, eksik host yok.
- Gerçek sebep görünürlüktü: geçişte gösterilen **"Yükleniyor…" katmanı** yalnızca
  sayfa yüklenmesi bittiğinde kaldırılıyordu. Hedef sayfa hiç yüklenmezse (ağ hatası,
  403, sunucu yanıt vermiyor) katman **kalıcı** olarak ekranı kaplıyor ve kullanıcı
  bunu "geçiş yok / uygulama dondu" olarak görüyordu.
- Katman artık en geç **15 saniye** sonunda kaldırılıyor. Ayrıca hedef uygulama 403/401
  dönüyorsa aşağıdaki erişim ekranı çıkıyor.

### Yeni — Erişimi engellenmiş kullanıcıya net mesaj
- "Uygulamalar" menüsünden geçiş sonrası hedef adres bir kez daha sorgulanır (HEAD,
  desteklenmiyorsa GET). Sunucu **403/401** dönüyorsa beyaz/404 sayfa yerine
  **"Erişim izniniz yok — Bu uygulamaya geçiş izniniz yok. Yöneticiniz bu uygulamaya
  erişiminizi kapatmış olabilir."** ekranı ve bir **"Geri dön"** düğmesi gösterilir.

### Yeni — İndirme konumu şeffaflığı
- Bildirim artık dosyanın **tam konumunu** yazıyor ("İndirildi: rapor.pdf" /
  "Konum: /Users/…/Downloads").
- Sayfada da kısa bir bilgi mesajı çıkıyor: "İndirildi: rapor.pdf → /…/Downloads".
- Tepsi menüsündeki **"Son indirilen dosyayı göster"** artık klasörü açmakla kalmıyor,
  dosyayı **seçili** gösteriyor (macOS `open -R` / Windows `explorer /select,`).
- **Yapılmadı (bilinçli):** "İndirme klasörünü seç" ayarı. Klasör seçici için gereken
  `FilePath` → `PathBuf` dönüşümü doğrulanamadı ve derleme yapılamadığı için riskli
  bulundu; konum şeffaflığı yerine geçici çözüm olarak sunuldu.

### Yeni — Yazdırma (Cmd/Ctrl+P)
- Görünüm menüsüne **"Yazdır…" (Cmd/Ctrl+P)** eklendi.
- macOS'ta WebView'in **native yazdırma diyalogu** (`WebviewWindow::print()`) kullanılır;
  wry bu API'yi yalnızca macOS'ta destekler. Windows/Linux'ta sayfanın kendi
  `window.print()` akışı çağrılır (wry belgelerine göre tüm platformlarda çalışır).
  **Her durumda tek bir yazdırma diyalogu açılır.**
- Panelin kendi "Yazdır" düğmeleri için sayfanın gerçek `print` fonksiyonu
  `window.__bogahostNativePrint` olarak saklanır.
- Önizleme penceresindeki PDF, o pencerenin kendi **"Yazdır"** düğmesiyle yazdırılır.

### Değişti
- `on_download` gövdesi `download_requested` / `download_finished` fonksiyonlarına
  ayrıldı; ana pencere ve önizleme pencereleri **aynı** indirme mantığını kullanıyor.
- `capabilities/default.json` ve `capabilities/remote.json` artık `popup-*` pencere
  desenini de kapsıyor (önizleme pencerelerinde köprü çalışsın diye).

## 1.4.0

### Yeni
- **Açılışta yükleme ekranı (splash).** Önceden ana pencere gizli başlıyor ve ancak
  uzak sayfa yüklenince gösteriliyordu; kullanıcı birkaç saniye boyunca hiçbir şey
  görmediği için uygulama açılmamış sanıyordu. Artık ana pencereden **önce** koyu
  temalı (`#0e1015` / vurgu `#5443D2`) küçük, çerçevesiz bir yükleme penceresi
  açılıyor: uygulama işareti, uygulama adı, "Yükleniyor…" ve ince bir ilerleme
  animasyonu. Bu pencere yerel `dist/index.html` sayfasını gösterir — ağ olmasa
  bile **anında** görünür.
  Uzak sayfa yüklenince (`on_page_load` → `Finished`) yükleme ekranı kapanır ve
  ana pencere gösterilir. Sayfa hiç açılmazsa mevcut **8 saniyelik emniyet ağı**
  yine devrededir; yükleme ekranı sonsuza kadar kalmaz.
- **Giriş ekranında sürüm rozeti.** Sayfaya enjekte edilen betik, giriş sayfasının
  altında `v<sürüm> · Powered by Bogahost` yazan küçük, düşük opaklıklı bir rozet
  gösterir. Sürüm Rust tarafından (`CARGO_PKG_VERSION`) gelir ve JS'e
  `window.__BOGAHOST_NATIVE_VERSION__` olarak aktarılır — böylece kullanıcı hangi
  native sürümü kurduğunu görebilir.
  Rozet **yalnızca giriş sayfasında** çıkar (adreste `/login`/`/giris` geçiyorsa
  veya sayfada parola alanı varsa); panel arayüzünde gösterilmez. Tıklanamaz
  (`pointer-events:none`), koyu ve açık temada okunur ve panelin kendi öğelerini
  kapatmaz. Aynı rozet yükleme ekranında da görünür.

### Değişti
- `on_window_event` artık yalnızca `main` etiketli pencere için çalışıyor: yükleme
  ekranının konumu/boyutu ana pencerenin kayıtlı durumunun üzerine yazamaz ve
  yükleme ekranı kapatılırken "tepsiye gizle" davranışı tetiklenmez.

## 1.3.0

### Yeni
- **Otomatik güncelleme aktif.** Updater public key'i (`5F2EECE9CC8FB8FD`) yapılandırmaya
  eklendi. Bundan sonra uygulama yeni sürümü kendisi denetler, onay isteyip indirir,
  kurar ve yeniden başlatır — elle indirme/kurma gerekmez.

### Değişti
- `apply-updater-config.mjs` artık commit edilmiş public key'i de kabul ediyor;
  ayrı bir `TAURI_SIGNING_PUBLIC_KEY` secret'ı zorunlu değil.

## 1.2.1

### Düzeltildi
- **macOS: pencere kapatılınca Dock'ta kalıyor ama tıklayınca açılmıyordu.** Uygulama
  kapatmada tepsiye gizleniyor (doğru davranış), ancak macOS'un Dock tıklamasında
  gönderdiği `Reopen` olayı hiç işlenmiyordu; bu yüzden pencere bir daha geri gelmiyordu.
  Artık Dock ikonuna tıklayınca pencere gösterilip öne getiriliyor.

# Değişiklik Günlüğü

Bu depo Bogahost'un 4 sisteminin (Finans / DCIM / Chat / Görevler) native
kabuklarını içerir: Capacitor 7 (Android/iOS) + Tauri 2 (Windows/macOS).
Sürüm numarası 4 uygulamada ve tüm platformlarda ortaktır.

## [1.2.0] — 2026-07-20

Kullanıcı geri bildirimi üzerine masaüstü (Tauri) kabuklarına odaklanan sürüm:
**indirmeler çalışmıyordu, uygulama donuyordu ve "uygulama gibi" davranmıyordu.**
Dört uygulamada da (Finans / DCIM / Chat / Görevler) `lib.rs` birebir aynıdır.

### Eklendi — İndirme desteği (en kritik)
- **Panelden üretilen PDF/CSV dosyaları artık gerçekten iniyor.** Ana pencere
  `WebviewWindowBuilder` ile kuruluyor ve `on_download` olayına bağlanıyor:
  dosya **İndirilenler** klasörüne yazılır, ad çakışmasında `-1`, `-2` … eklenir.
  (Rapor PDF/CSV, teklif PDF, hediye listesi CSV, Paraşüt fatura PDF vb.)
- **İndirme bitince native bildirim:** "İndirildi: `<dosya>`". Tepsi menüsüne
  **"İndirilenler klasörünü aç"** ve **"Son indirilen dosyayı göster"** eklendi
  (dosyanın bulunduğu klasör Finder/Gezgin'de açılır).
- **`blob:` / `data:` indirmeleri** (tarayıcıda üretilen dosyalar) için sayfaya
  enjekte edilen köprü: dosya `bogahost_save_file` komutuyla diske yazılır.
  Köprü çalışmazsa WebView'in kendi indirme akışına düşülür.
- **WebView'de gösterilemeyen türler** (ör. `window.open` ile açılan PDF)
  kaydedilip **sistem uygulamasında** açılır.
- **`target="_blank"` linkleri artık sessizce yutulmuyor:** aynı pencerede açılır;
  adres kendi alan adımız (`*.bogahost.com`) dışındaysa **sistem tarayıcısına**
  yönlendirilir (`on_navigation`).

### Düzeltildi — Donma / takılma
- **Açılışta hiçbir ağ çağrısı `setup()`'ı bloklamıyor.** Sürüm denetimi 5 sn
  gecikmeyle, bildirim izni 2 sn gecikmeyle arka planda çalışır.
- **Ağ işlemlerine 7 sn zaman aşımı** (updater + yedek manifest); başarısızsa sessizce geçilir.
- **Bildirimler ana thread'e kuyruklanıyor** (`run_on_main_thread`) — macOS
  bildirim API'sinin arka thread'den çağrılması kaynaklı takılmalar giderildi.
- **Beyaz/donuk açılış karesi yok:** pencere gizli başlar, sayfa yüklenince
  gösterilir; sayfa hiç yüklenmezse en geç 8 sn sonra yine gösterilir.
- Menü/tepsi işleyicilerinde uzun süren iş yapılmaz.

### Eklendi — "Uygulama gibi" davranış
- **Standart macOS menüleri artık açıkça kuruluyor** (kaybolma riski yok):
  Uygulama (Hakkında, Hizmetler, Gizle **Cmd+H**, Çıkış **Cmd+Q**),
  **Düzen** (Geri Al/Yinele, Kes **Cmd+X**, Kopyala **Cmd+C**, Yapıştır **Cmd+V**,
  Tümünü Seç **Cmd+A**), **Pencere** (Küçült **Cmd+M**, Kapat **Cmd+W**).
- **Yeni "Görünüm" menüsü:** Yenile **Cmd+R**, Geri **Cmd+[**, İleri **Cmd+]**,
  Yakınlaştır **Cmd+=**, Uzaklaştır **Cmd+-**, Gerçek Boyut **Cmd+0**, Tam Ekran.
  Aynı öğeler Windows için tepsi menüsüne de eklendi.
- **Pencere boyutu/konumu hatırlanıyor** (uygulama yapılandırma klasöründe
  `window-state.json`).
- Klavye ile yakınlaştırma (`zoom_hotkeys_enabled`) açık.

### Eklendi — Bildirim izni
- Tepsi menüsünde **"Bildirimler: açık / kapalı"** göstergesi; kapalıyken
  tıklandığında **sistem bildirim ayarları** açılır (macOS/Windows).
- İlk açılıştaki tek seferlik tanıtım bildirimi (izin penceresini tetikler) korundu,
  artık arka planda ve bloklamadan çalışır.

### Değişti
- Ana pencere tanımı `tauri.conf.json > app.windows` yerine **Rust tarafında**
  (`build_main_window`) oluşturuluyor — `on_download` / `on_navigation` /
  `on_page_load` yalnızca bu yolla bağlanabiliyor.
- Uygulamalar arası geçişte **"Yükleniyor…" katmanı** gösteriliyor; geçiş sonrası
  pencere başlığı ve menü işaretleri güncelleniyor (mevcut davranış korundu).
- Tüm sürüm alanları **1.2.0**.

### Korundu
- Tepsi ikonu ve kapatınca tepsiye gizlenme, otomatik güncelleme akışı
  (imzasızken çökmeme dahil), `remote.json` ile yalnızca bildirim izni,
  eski manifest yolu (`https://bogahost.com/native/latest.json`), CI yapısı.

---

## [Yayınlanmamış]

### Eklendi — Tam otomatik güncelleme (masaüstü)
- **`tauri-plugin-updater` 4 uygulamada da devrede.** Açılışta sessiz denetim →
  güncelleme varsa **onay diyaloğu** ("Yeni sürüm X hazır. Şimdi kurulsun mu?") →
  onaylanırsa **indir + imza doğrula + kur + yeniden başlat**. Tepsideki
  "Güncellemeleri denetle" aynı akışı elle tetikler. "Daha sonra" denirse kalıcı
  kayıt tutulmaz; bir sonraki açılışta tekrar sorulur.
- **Endpoint şeması:** `https://native.bogahost.com/updates/<app>/{{target}}/{{arch}}/latest.json`
  (yedek: `.../updates/<app>/latest.json`). Per-target/arch dosyaları birincildir —
  `windows.yml` ve `macos.yml` ayrı koştuğu için ortak dosyada çakışma olmaz.
- **`tauri-plugin-dialog`** onay diyaloğu için eklendi.
- **Dayanıklılık:** Updater eklentisi `setup()` içinde kayıt edilir; `pubkey`
  PLACEHOLDER/bozuksa eklenti yüklenmez, hata yutulur ve uygulama **eski manifest
  denetimine** düşer. `https://bogahost.com/native/latest.json` yolu (v1.1.0
  istemcileri için) **korunmuştur**. Hiçbir hata uygulamayı kilitlemez/çökertmez.

### Eklendi — CI: imzalama + yayınlama
- **`.github/workflows/generate-updater-key.yml`** (yalnız `workflow_dispatch`):
  minisign anahtar çifti üretir; **public key** iş özetine yazılır, **private key**
  yalnızca artifact olarak çıkar — log'a asla yazılmaz. Depoya anahtar konmaz.
- **`scripts/ci/apply-updater-config.mjs`:** build öncesi `pubkey` enjeksiyonu +
  `createUpdaterArtifacts`. Secret yoksa dosyaya DOKUNMAZ → imzasız build yeşil kalır.
- **`scripts/ci/publish-updates.mjs`:** installer + `.sig` + `latest.json` +
  `index.html` üretir; sunucudaki kopyayı okuyup birleştirir (okunamazsa üzerine
  yazmaz). Android APK `downloads/`'a yüklenir (otomatik güncelleme yok).
- **FTPS yayın** (`lftp`, `ftp:ssl-force true`) — `DEPLOY_FTP_HOST/USER/PASS`.
  Secret yoksa adım ATLANIR (uyarı verir, CI kırmızıya dönmez).
- **Boş env tuzağına karşı:** `TAURI_SIGNING_*` env'leri yalnızca DOLUYSA export
  edilir (macOS'taki "failed to import keychain certificate" ile aynı sınıf hata).
- **Authenticode + updater çakışması çözüldü:** Authenticode installer baytlarını
  değiştirdiği için `.sig` sonrasında `tauri signer sign` ile yeniden üretilir.

### Değişti
- `docs/UPDATE.md` tam otomatik akışa göre **yeniden yazıldı** (mimari, şema,
  secret tablosu, anahtar üretimi, **Cloudflare grey-cloud zorunluluğu**, sorun giderme).
- `docs/latest.json.example` Tauri updater şemasına güncellendi;
  `docs/latest-legacy.json.example` eski şema için eklendi.

### Gerekli manuel adımlar
- Anahtar üretme workflow'unu bir kez çalıştırıp `TAURI_SIGNING_*` secret'larını ekleyin.
- `public_html/native` köküne işaret eden bir FTP hesabı açıp `DEPLOY_FTP_*` ekleyin.
- `native.bogahost.com` için Cloudflare'i **DNS-only (gri bulut)** yapın veya bypass
  kuralı tanımlayın — aksi hâlde updater 403 alır.

## [1.1.0] — 2026-07-20

### Eklendi — Uygulamalar arası geçiş
- **Masaüstü (Tauri):** Sistem tepsisi menüsüne ve **macOS menü çubuğuna**
  **"Uygulamalar"** alt menüsü eklendi: *Finans · DCIM · Chat · Görevler*.
  Seçilen uygulama **mevcut pencerede** açılır (`WebviewWindow::navigate`) —
  yeni pencere açılmaz. Aktif uygulama menüde **işaretli ve pasif (gri)** görünür
  ve geçişten sonra işaret güncellenir; pencere başlığı da değişir.
  - macOS'ta varsayılan menü (Uygulama/Düzen/Pencere) korunur, alt menü sonuna eklenir.
  - Windows'ta pencere içinde menü çubuğu yoktur; geçiş tepsi menüsünden yapılır.
- **`tauri.conf.json` CSP genişletildi:** `default-src/connect-src/script-src/style-src/
  img-src/font-src/media-src/frame-src` artık **4 host'u da** (`finans|dcim|chat|task
  .bogahost.com`) ve `wss://` karşılıklarını kapsıyor. Aksi hâlde geçiş sonrası sayfa
  kaynakları engellenirdi.
- **Mobil (Capacitor):** `server.allowNavigation` artık 4 host'u da içeriyor; böylece
  aynı kabukta sistemler arası geçiş mümkün. Android `network_security_config.xml`
  tek host yerine `bogahost.com` (alt alan adları dâhil) için tanımlandı.
  Mobilde ayrı bir menü eklenmedi — geçiş, panellerin kendi arayüzünden yapılır.

### Eklendi — Bildirim izni (macOS/Windows)
- Uygulama açılışında bildirim izni **açıkça** isteniyor
  (`permission_state` → gerekiyorsa `request_permission`).
- İzin alındıktan sonra **yalnızca ilk çalıştırmada** "Bildirimler açıldı" test
  bildirimi gösteriliyor (uygulama veri klasöründeki bir bayrak dosyası ile tekrarı engelleniyor).
  macOS'ta sistem onay penceresini tetikleyen şey bu ilk bildirimdir.
- `capabilities/default.json` dosyalarına `notification:default`,
  `notification:allow-notify`, `notification:allow-is-permission-granted`,
  `notification:allow-request-permission` izinleri eklendi
  (uzak origin izinleri `capabilities/remote.json` içinde zaten vardı).

### Eklendi — Güncelleme denetimi
- Masaüstü uygulamaları açılışta **sessizce** `https://bogahost.com/native/latest.json`
  adresini kontrol eder; yeni sürüm varsa native bildirim gösterir.
- Tepsi menüsüne **"Güncellemeleri denetle"** ve **"İndirme sayfasını aç"** eklendi.
- Tam otomatik (indir-kur) updater **bilinçli olarak eklenmedi**: depo private olduğu
  için release asset'leri anonim indirilemez ve imzalama anahtarı gerekir.
  Gerekçe, `latest.json` biçimi, yayın akışı ve ileride tam updater'a geçiş adımları:
  **`docs/UPDATE.md`**.

### Değişti
- Tüm sürüm alanları `1.0.0` → `1.1.0`
  (kök `package.json`, `apps.config.json`, `tauri/*/package.json`,
  `tauri/*/src-tauri/tauri.conf.json`, `tauri/*/src-tauri/Cargo.toml`,
  `capacitor/*/package.json`).
- `apps.config.json` artık bir `version` alanı taşıyor.
- Tauri kabuklarına `reqwest` bağımlılığı eklendi (sürüm denetimi için;
  platformun kendi TLS'ini kullanır, ek sistem bağımlılığı gerektirmez).

### Bilinen sınırlar / dürüst notlar
- macOS uygulaması **imzasız/notarize edilmemiş** dağıtılıyor. Bildirimler çalışır
  ancak ilk açılışta Gatekeeper uyarısı çıkar (sağ tık → Aç, ya da
  Sistem Ayarları → Gizlilik ve Güvenlik → "Yine de Aç").
  İmzasız uygulamalarda macOS bildirim davranışı sürüme göre değişebilir —
  garanti verilmiyor (bkz. `tauri/README.md`).
- Güncelleme bildirimi, `latest.json` canlı siteye **elle** yüklenene kadar çıkmaz.
- Web-push (Service Worker) masaüstü WebView'lerinde güvenilir çalışmaz; masaüstünde
  bildirim için native köprü kullanılır.

## [1.0.0]

- İlk sürüm: 4 uygulama × Android (APK/AAB), iOS (arşiv), Windows (MSI/NSIS),
  macOS (DMG/.app) kabukları; sistem tepsisi, kapatınca gizle, native bildirim
  köprüsü, CI iş akışları (GitHub Actions) ve PWA iyileştirme parçacıkları.
