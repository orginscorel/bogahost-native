//! Bogahost Kasa — masaüstü programlarına güvenli kimlik doldurma.
//!
//! NEDEN DİĞER UYGULAMALARDAN FARKLI: finans/dcim/chat/task/muh kabukları canlı
//! bir URL yükleyen WebView'lerdir. Kasa bunu yapamaz; işini görebilmesi için
//! işletim sistemi yeteneklerine ihtiyacı var — küresel kısayol, öndeki
//! pencereyi okuma ve BAŞKA bir programa tuş gönderme. Bu yüzden arayüz
//! yereldir (uzak URL değil) ve iş mantığı Rust tarafındadır.
//!
//! TARAYICI DAHİL HER PENCEREYE YAZAR. Klavye girdisi olarak gönderildiği için
//! hedefin ne olduğu fark etmez; ayrı bir tarayıcı istisnası yoktur.
//!
//! KOMUTLAR NEDEN `#[tauri::command(async)]`:
//! Tauri'de `async` İŞARETİ OLMAYAN komutlar ANA İŞ PARÇACIĞINDA çalışır.
//! Buradaki komutların çoğu ağa çıkıyor (20 sn zaman aşımı) ve `doldur` ayrıca
//! uyuyor; ana iş parçacığında çalışsalardı pencere o süre boyunca donardı.
//! `(async)` işareti gövdeyi ayrı bir görevde çalıştırır.
//!
//! DURUM NEDEN `AppHandle` ÜZERİNDEN OKUNUYOR:
//! `State<'_, T>` ödünç alınmış bir argümandır; komut ayrı bir göreve taşınınca
//! ömrü karışır. `AppHandle` 'static ve klonlanabilir olduğu için durum gövde
//! İÇİNDE `uygulama.state::<Durum>()` ile alınır — ödünç, gövdeden dışarı çıkmaz.

mod alan;
mod anahtarlik;
mod ayarlar;
mod guncelleme;
mod kasa;
mod kopru;
mod kripto;
mod politika;
mod pencere;

use kasa::{Durum, GirisSonuc, Kayit};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Kısayola basıldığı ANDAKİ hedef. Kullanıcı listeden seçim yaparken odak
/// bizim penceremize geçer; yazarken kullanılacak pencere bu değil, kısayol
/// anında yakalanan penceredir.
#[derive(Default)]
struct HedefDurum(Mutex<pencere::Hedef>);

/// Bellekteki kripto anahtarları. Diske yazılmıyor, sunucuya gitmiyor,
/// arayüze verilmiyor — yalnız burada.
struct KilitDurum(Mutex<anahtarlik::Anahtarlik>);

impl Default for KilitDurum {
    fn default() -> Self {
        // Süre AYARDAN geliyor; sabit bir varsayılan yalnız ayar okunamazsa.
        Self(Mutex::new(anahtarlik::Anahtarlik::yeni(
            ayarlar::oku().otomatik_kilit_dk,
        )))
    }
}

#[derive(Serialize)]
struct DoldurSonuc {
    tamam: bool,
    mesaj: String,
}

// ── Komutlar ───────────────────────────────────────────────────────────────

#[tauri::command(async)]
fn giris_yap(uygulama: tauri::AppHandle, kullanici: String, sifre: String) -> GirisSonuc {
    let cihaz = format!(
        "{} — {}",
        bilgisayar_adi(),
        if cfg!(windows) { "Windows" } else { "macOS" }
    );
    kasa::giris(&uygulama.state::<Durum>(), &kullanici, &sifre, &cihaz)
}

#[tauri::command(async)]
fn kod_dogrula(uygulama: tauri::AppHandle, kod: String) -> GirisSonuc {
    kasa::dogrula(&uygulama.state::<Durum>(), &kod)
}

#[tauri::command(async)]
fn oturum_var(uygulama: tauri::AppHandle) -> bool {
    kasa::oturum_gecerli(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn oturumu_kapat(uygulama: tauri::AppHandle) {
    kasa::jeton_sil();
    *uygulama.state::<Durum>().jeton.lock().unwrap() = None;
}

#[tauri::command(async)]
fn kayitlar(uygulama: tauri::AppHandle) -> Result<Vec<Kayit>, String> {
    kasa::liste(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn hedef(uygulama: tauri::AppHandle) -> pencere::Hedef {
    uygulama.state::<HedefDurum>().0.lock().unwrap().clone()
}

#[derive(Serialize)]
struct Izinler {
    /// Pencere adlarını okumak / pencereyi öne getirmek (macOS: Apple Events)
    otomasyon: bool,
    /// Başka programa tuş göndermek (macOS: Accessibility)
    erisilebilirlik: bool,
}

/// İki izin AYRI AYRI sorulur. Biri verilip diğeri unutulduğunda ekranda
/// "hiç pencere yok" gibi yanıltıcı bir durum oluşuyordu; artık hangisinin
/// eksik olduğunu söyleyebiliyoruz.
#[tauri::command(async)]
fn izinler() -> Izinler {
    Izinler {
        otomasyon: pencere::otomasyon_izni_var(),
        erisilebilirlik: pencere::erisilebilirlik_izni_var(),
    }
}

/// Açık programların listesi — kullanıcı hedefi buradan seçebilsin diye.
///
/// NEDEN GEREKLİ: hedef yalnızca küresel kısayolla yakalanabiliyordu. Kullanıcı
/// uygulamayı doğrudan açtığında ya da Otomasyon izni verilmemişken ekranda
/// "Hedef pencere seçilmedi" yazıyor ve seçilecek hiçbir şey bulunmuyordu.
#[tauri::command(async)]
fn pencereler() -> Vec<pencere::Hedef> {
    pencere::listele()
}

/// Listeden seçilen pencereyi hedef yapar.
#[tauri::command(async)]
fn hedef_sec(uygulama: tauri::AppHandle, kimlik: String) -> Option<pencere::Hedef> {
    let secilen = pencere::listele().into_iter().find(|h| h.kimlik == kimlik)?;
    *uygulama.state::<HedefDurum>().0.lock().unwrap() = secilen.clone();
    Some(secilen)
}

/// Kullanıcı adını ve parolayı hedef pencereye YAZAR.
///
/// Parola buradan dışarı çıkmaz: sunucudan alınır, klavyeye yazılır, işlev
/// biterken bellekten düşer. Arayüz parolayı hiç görmez.
#[tauri::command(async)]
fn doldur(uygulama: tauri::AppHandle, id: i64, enter_bas: bool) -> DoldurSonuc {
    if !pencere::erisilebilirlik_izni_var() {
        return DoldurSonuc {
            tamam: false,
            mesaj: "macOS Erişilebilirlik izni gerekiyor: Sistem Ayarları → Gizlilik ve Güvenlik → Erişilebilirlik → Bogahost Kasa".into(),
        };
    }

    let h = uygulama.state::<HedefDurum>().0.lock().unwrap().clone();
    if h.program.is_empty() {
        return DoldurSonuc {
            tamam: false,
            mesaj: "Hedef pencere seçilmedi. Listeden bir program seçin ya da doldurulacak pencere öndeyken kısayola basın.".into(),
        };
    }

    // HEDEF ↔ KAYIT EŞLEŞMESİ — parola yanlış siteye ASLA gitmesin.
    //
    // Eskiden burada hiçbir kontrol yoktu: kullanıcı A kaydını seçip B sitesine
    // Doldur diyebiliyordu ve parola B'ye yazılıyordu. Hedefin adresi
    // okunabiliyorsa (tarayıcı) kaydın adresiyle karşılaştırılıyor; uymuyorsa
    // yazılmıyor.
    //
    // Masaüstü programlarında adres diye bir şey yok, karşılaştıracak bir
    // şey de yok — orada kullanıcının kaydı seçip Doldur demesi zaten açık
    // niyet beyanıdır ve engellenmiyor.
    if let Some(hedef_url) = h.url.clone() {
        let kayitlar = kasa::liste(&uygulama.state::<Durum>()).unwrap_or_default();
        if let Some(k) = kayitlar.iter().find(|k| k.id == id) {
            if !yol_uyar(&k.url, &hedef_url) {
                return DoldurSonuc {
                    tamam: false,
                    mesaj: format!(
                        "\"{}\" bu adrese ait değil ({}). Parola yazılmadı.",
                        k.label, hedef_url
                    ),
                };
            }
        }
    }

    /* TARAYICIDA TUŞ GÖNDERMİYORUZ — EKLENTİ DOLDURUYOR.
       Kısayol yolunda bu kural zaten vardı ama panelden "Doldur"a basınca
       yine tuş gönderiliyordu. Sayfanın içini göremeyen bir program hangi
       kutunun ne olduğunu ancak tahmin edebilir; bildirilen "rastgele bir
       inputa şifre yazıyor" hatasının kökü bu.
       Eklenti sayfayı GÖRÜYOR: parola alanını bulur, altında menüsünü açar,
       oraya yazar. Doğru iş bölümü bu. */
    if h.tarayici {
        /* PANELDEN TIKLANINCA EKLENTİYİ TETİKLİYORUZ.
           Eskiden burada "doldurmayı eklenti yapıyor" yazan bir metin
           dönüyordu. Bu bir cevap değil bahaneydi: kullanıcı paneli görüyor,
           kaydı görüyor, tıklıyor ve hiçbir şey olmuyordu.
           Parola bu yoldan GEÇMİYOR — köprüye yalnız "şu kaydı şu adreste
           doldur" yazılıyor; eklenti sonra her zamanki gibi kendi isteyip
           alıyor ve sayfayı GÖREREK dolduruyor. */
        let Some(url) = h.url.clone() else {
            return DoldurSonuc {
                tamam: false,
                mesaj: "Tarayıcının adresi okunamadı. Sekmeye tıklayıp tekrar deneyin.".into(),
            };
        };

        if !kopru::eklenti_bagli() {
            return DoldurSonuc {
                tamam: false,
                mesaj: "Tarayıcıda doldurmayı eklenti yapıyor ama eklenti bağlı değil. \
                        Ayarlar → Chrome eklentisi'nden kurun."
                    .into(),
            };
        }

        kopru::olay_ekle(serde_json::json!({
            "tur": "doldur", "id": id, "url": url,
        }));

        /* BURADA "DOLDURULDU" DEMİYORUZ.
           İşi kuyruğa bırakmak, doldurulduğu anlamına gelmiyor: eklenti
           sekmeyi bulamayabilir, sayfada alan olmayabilir. Burada
           işaretleseydik iş başarısız olsa bile panel o sitede yirmi dakika
           susardı. Damgayı eklenti GERÇEKTEN doldurduğunda köprü basıyor
           (`doldur_cevabi`). */

        if let Some(p) = uygulama.get_webview_window("hizli") {
            let _ = p.hide();
        }
        return DoldurSonuc { tamam: true, mesaj: "Eklentiye iletildi.".into() };
    }

    // Doldurma /fill ucundan geçer: gizli kayıtta görüntüleme kapalı olsa
    // bile yazma çalışır ve her yazım denetime düşer.
    let (kullanici, sifre) = match kasa::doldurmak_icin_ac(&uygulama.state::<Durum>(), id) {
        Ok(v) => v,
        Err(e) => return DoldurSonuc { tamam: false, mesaj: format!("Alınamadı: {e}") },
    };

    // SIRA ÖNEMLİ: ÖNCE hedefi öne getir, SONRA kendi penceremizi gizle.
    // Windows'ta SetForegroundWindow yalnızca çağıran süreç ÖN PLANDAYKEN
    // çalışır; kendimizi önce gizlersek o hakkı kaybeder ve tuşlar yanlış
    // pencereye gider. Gizleme ikinci adımda, hedef zaten odaktayken yapılır.
    if !h.kimlik.is_empty() {
        pencere::one_getir(&h.kimlik);
        std::thread::sleep(std::time::Duration::from_millis(240));
    }

    // HANGİ PENCERE AÇIKSA O GİZLENİR.
    // Doldurma hızlı erişim penceresinden de tetikleniyor; yalnızca ana
    // pencereyi gizlemek, hızlı pencere ekranda kalıp odağı tutması ve
    // tuşların oraya gitmesi demekti.
    let pencere_tauri = uygulama
        .get_webview_window("hizli")
        .filter(|p| p.is_visible().unwrap_or(false))
        .or_else(|| uygulama.get_webview_window("main"));
    if let Some(p) = &pencere_tauri {
        let _ = p.hide();
    }
    std::thread::sleep(std::time::Duration::from_millis(140));

    // ALAN DENETİMİ — yazmadan önce.
    // Klavye odakta ne varsa oraya yazar; adresi eşleşen bir sayfada odak
    // arama kutusundaysa parola oraya gider. Dolu alanın üstüne yazmak da
    // veri kaybıdır. Tespit edilemiyorsa (Windows / izin yok) engellemiyoruz.
    let mut uygun = alan::odakli_alan();

    // ODAK METİN ALANINDA DEĞİLSE ALANI ARA.
    //
    // Bildirilen durum: "Odakta bir metin alanı yok (AXButton)". Doğruydu ama
    // işe yaramıyordu — kullanıcı panelden Doldur'a bastıysa niyeti zaten
    // belli; ondan ayrıca doğru kutuya tıklamasını beklemek gereksiz bir adım.
    // Tab ile ileri gidip ilk BOŞ metin alanını buluyoruz. Hiçbir şey
    // yazılmıyor, yalnızca odak ilerliyor; bulunamazsa eski davranışa dönülüp
    // kullanıcıya söyleniyor.
    if matches!(uygun, alan::Uygun::AlanDegil(_)) {
        if let Ok(mut e) = enigo::Enigo::new(&enigo::Settings::default()) {
            use enigo::{Direction, Key, Keyboard};
            for _ in 0..8 {
                if e.key(Key::Tab, Direction::Click).is_err() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(90));
                uygun = alan::odakli_alan();
                if uygun == alan::Uygun::Bos {
                    break;
                }
            }
        }
    }

    if uygun == alan::Uygun::Dolu || matches!(uygun, alan::Uygun::AlanDegil(_)) {
        if let Some(p) = &pencere_tauri {
            let _ = p.show();
        }
        return DoldurSonuc { tamam: false, mesaj: alan::aciklama(&uygun) };
    }

    let sonuc = yaz(&kullanici, &sifre, enter_bas);

    // Parola bellekten düşsün (Rust burada zaten bırakır; niyet açık olsun diye)
    drop(sifre);

    if let Some(p) = &pencere_tauri {
        let _ = p.show();
    }

    match sonuc {
        Ok(()) => {
            // Bu SİTE için panel bir süre kendiliğinden açılmasın.
            if let Some(u) = &h.url {
                dolduruldu_isaretle(u);
            }
            DoldurSonuc { tamam: true, mesaj: "Dolduruldu.".into() }
        }
        Err(e) => DoldurSonuc {
            tamam: false,
            mesaj: format!("Yazılamadı: {e}. Kopyala düğmelerini kullanabilirsiniz."),
        },
    }
}

// ── Tarayıcı koruması ──────────────────────────────────────────────────────

/// Politikanın durumu — açılışta okunur, yönetici hakkı İSTEMEZ.
#[tauri::command(async)]
fn politika_durum() -> politika::Durum {
    politika::durum()
}

/// Eklentiyi Chrome'un HARİCİ EKLENTİ mekanizmasıyla kur.
///
/// Zorunlu kurulum politikası Chrome'un iznine bağlı ve mağaza dışı
/// eklentilerde Chrome zorluk çıkarıyor. Bu ikinci yol, CRX'i makineye indirip
/// Chrome'un okuduğu klasöre bir kayıt dosyası bırakıyor; Chrome bir sonraki
/// açılışta kuruyor. İkisi çakışmaz, birbirini tamamlar.
#[tauri::command(async)]
fn eklenti_kur() -> Result<(), String> {
    politika::eklenti_kur()
}

#[tauri::command(async)]
fn eklenti_kaldir() -> Result<(), String> {
    politika::eklenti_kaldir()
}

/// Harici kurulum dosyası yerinde mi? Arayüz buna göre düğme gösteriyor.
#[tauri::command(async)]
fn eklenti_dosyasi_var() -> bool {
    politika::eklenti_dosyasi_var()
}

/// Eklenti paketini İndirilenler'e indirip klasörü açar — yönetici hakkı
/// istemez. Politika ya da harici kurulum tutmadığında kalan yol bu.
#[tauri::command(async)]
fn eklenti_indir() -> Result<String, String> {
    politika::eklenti_indir()
}

/// Yerel köprünün durumu — eklenti bu bilgisayardaki oturumu buradan kullanır.
#[tauri::command(async)]
fn kopru_durum() -> serde_json::Value {
    kopru::durum()
}

/// Chrome'daki eklenti kaç numaralı sürüm, sunucuda kaç numaralı?
///
/// NEDEN GEREKLİ: "Paketlenmemiş öğe yükle" ile kurulan eklenti kendi kendine
/// GÜNCELLENMEZ — Chrome açılışta klasördeki dosyaları okur, `update.xml`e
/// hiç bakmaz. Bildirilen durum buydu: uygulama güncellendi, eklenti eski
/// kaldı. Artık iki sürüm de ekranda yazıyor.
#[tauri::command(async)]
fn eklenti_surum_durum() -> serde_json::Value {
    let yerel = politika::eklenti_yerel_surum();
    let uzak = politika::eklenti_uzak_surum();
    let guncel = match (&yerel, &uzak) {
        (Some(y), Some(u)) => y == u,
        _ => false,
    };
    let klasor = politika::eklenti_klasoru().map(|k| k.display().to_string());
    serde_json::json!({ "yerel": yerel, "uzak": uzak, "guncel": guncel, "klasor": klasor })
}

/// Klasördeki eklentiyi sunucudaki sürümle değiştirir.
#[tauri::command(async)]
fn eklenti_tazele() -> Result<Option<String>, String> {
    politika::eklenti_tazele()
}

/// Yüklü profili kaldır — güncellemede eskisini elle silmek gerekmesin.
#[tauri::command(async)]
fn politika_profil_kaldir() -> Result<politika::Durum, String> {
    politika::profil_kaldir()?;
    Ok(politika::durum())
}

/// Politikayı kur — işletim sisteminin kendi yönetici penceresi çıkar.
/// Kullanıcı parolasını bize değil işletim sistemine verir.
#[tauri::command(async)]
fn politika_kur() -> Result<politika::Durum, String> {
    politika::kur()?;
    Ok(politika::durum())
}

// ── Kayıt yönetimi ─────────────────────────────────────────────────────────
//
// Sunucu kuralı: herkes KENDİ eklediğini ekler, düzenler, siler; başkasının
// (yöneticinin) eklediğine dokunamaz. Karar sunucuda verilir; buradaki
// düğmeleri gizlemek yalnızca boşuna denemeyi önler.

#[tauri::command(async)]
fn kayit_ekle(uygulama: tauri::AppHandle, veri: serde_json::Value) -> Result<i64, String> {
    kasa::kayit_ekle(&uygulama.state::<Durum>(), veri)
}

#[tauri::command(async)]
fn kayit_guncelle(
    uygulama: tauri::AppHandle,
    id: i64,
    veri: serde_json::Value,
) -> Result<(), String> {
    kasa::kayit_guncelle(&uygulama.state::<Durum>(), id, veri)
}

#[tauri::command(async)]
fn kayit_sil(uygulama: tauri::AppHandle, id: i64) -> Result<(), String> {
    kasa::kayit_sil(&uygulama.state::<Durum>(), id)
}

// ── Faz 1: çöp kutusu, kayıt geçmişi, cihazlar ─────────────────────────────
//
// Üçü de denetimde çıkan somut kayıp/erişim risklerinin karşılığı:
//   Y-3  silme kalıcıydı        → çöp kutusu
//   O-2  eski parola kayboluyordu → kayıt geçmişi
//   K-3  jeton süresizdi         → cihaz listesi + iptal

#[tauri::command(async)]
fn cop(uygulama: tauri::AppHandle) -> Result<serde_json::Value, String> {
    kasa::cop(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn cop_geri(uygulama: tauri::AppHandle, id: i64) -> Result<(), String> {
    kasa::cop_geri(&uygulama.state::<Durum>(), id)
}

/// Kalıcı silme — geri dönüşü yok, o yüzden ayrı komut.
#[tauri::command(async)]
fn cop_kalici_sil(uygulama: tauri::AppHandle, id: i64) -> Result<(), String> {
    kasa::cop_kalici_sil(&uygulama.state::<Durum>(), id)
}

#[tauri::command(async)]
fn gecmis(uygulama: tauri::AppHandle, id: i64) -> Result<serde_json::Value, String> {
    kasa::gecmis(&uygulama.state::<Durum>(), id)
}

#[tauri::command(async)]
fn surume_don(uygulama: tauri::AppHandle, id: i64, revizyon: i64) -> Result<(), String> {
    kasa::surume_don(&uygulama.state::<Durum>(), id, revizyon)
}

#[tauri::command(async)]
fn cihazlar(uygulama: tauri::AppHandle) -> Result<serde_json::Value, String> {
    kasa::cihazlar(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn cihaz_iptal(uygulama: tauri::AppHandle, id: i64) -> Result<(), String> {
    kasa::cihaz_iptal(&uygulama.state::<Durum>(), id)
}

#[tauri::command(async)]
fn diger_cihazlari_kapat(uygulama: tauri::AppHandle) -> Result<i64, String> {
    kasa::diger_cihazlari_kapat(&uygulama.state::<Durum>())
}

/// SİSTEM BİLGİSİ — arayüz neyin ÇALIŞTIĞINI bilsin.
///
/// Ayarlar ekranı Windows'ta da "Yapılandırma profilini kaldır" gibi
/// düğmeler gösteriyordu; basılınca "bu platformda yok" hatası dönüyordu.
/// Çalışmayan bir düğme göstermek, kullanıcıyı kendi hatası sanmaya iter.
/// Artık arayüz ne göstereceğini buradan öğreniyor.
#[tauri::command(async)]
fn sistem() -> serde_json::Value {
    serde_json::json!({
        "platform": if cfg!(target_os = "macos") { "macos" }
                    else if cfg!(windows) { "windows" }
                    else { "diger" },
        /// macOS'ta Erişilebilirlik izni gerekiyor; Windows'ta böyle bir izin yok.
        "izin_gerekli": cfg!(target_os = "macos"),
        /// Yapılandırma profili yalnız macOS'ta var.
        "profil_destegi": cfg!(target_os = "macos"),
        /// Yönetici hakkıyla sistem geneli eklenti kurulumu — yalnız macOS.
        "yonetici_kurulumu": cfg!(target_os = "macos"),
        "eklenti_zip": politika::eklenti_zip_adresi(),
    })
}

#[tauri::command(async)]
fn ayarlar_oku() -> ayarlar::Ayar {
    ayarlar::oku()
}

/// Ayarları yaz ve ANINDA uygula.
///
/// Kaydedip "yeniden başlatın" demek, ayarı yarım uygulamaktır: kullanıcı
/// kilit süresini değiştirip aynı oturumda denemek ister.
#[tauri::command(async)]
fn ayarlar_yaz(uygulama: tauri::AppHandle, ayar: ayarlar::Ayar) -> Result<ayarlar::Ayar, String> {
    let a = ayar.duzelt();
    ayarlar::yaz(&a)?;
    {
        let durum = uygulama.state::<KilitDurum>();
        durum.0.lock().unwrap().kilit_suresini_ayarla(a.otomatik_kilit_dk);
    }
    Ok(a)
}

/* KİLİT DURUMU / KİLİTLE KOMUTLARI HENÜZ KAYITLI DEĞİL.
   `anahtarlik` modülü kullanılıyor (kilit süresi ayardan geliyor) ama kilidi
   AÇMA akışı — ana parola ekranı — Faz 3'ün kalan kısmı. Açma olmadan
   "kilitle" düğmesi göstermek, hiç kilitlenmemiş bir kasayı kilitliyormuş
   gibi yapmak olurdu. Komutlar akış gelince buraya dönecek. */

// ── Faz 2b: kasa ve üye yönetimi ───────────────────────────────────────────

#[tauri::command(async)]
fn kasalar(uygulama: tauri::AppHandle) -> Result<serde_json::Value, String> {
    kasa::kasalar(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn kasa_olustur(uygulama: tauri::AppHandle, ad: String, tur: String, aciklama: String)
    -> Result<serde_json::Value, String>
{
    kasa::kasa_olustur(&uygulama.state::<Durum>(), &ad, &tur, &aciklama)
}

#[tauri::command(async)]
fn kasa_sil(uygulama: tauri::AppHandle, id: i64) -> Result<(), String> {
    kasa::kasa_sil(&uygulama.state::<Durum>(), id)
}

#[tauri::command(async)]
fn kasa_uyeler(uygulama: tauri::AppHandle, id: i64) -> Result<serde_json::Value, String> {
    kasa::kasa_uyeler(&uygulama.state::<Durum>(), id)
}

#[tauri::command(async)]
fn uye_ekle(uygulama: tauri::AppHandle, kasa: i64, user_id: i64, rol: String) -> Result<(), String> {
    kasa::uye_ekle(&uygulama.state::<Durum>(), kasa, user_id, &rol)
}

#[tauri::command(async)]
fn uye_rol(uygulama: tauri::AppHandle, kasa: i64, user_id: i64, rol: String) -> Result<(), String> {
    kasa::uye_rol(&uygulama.state::<Durum>(), kasa, user_id, &rol)
}

#[tauri::command(async)]
fn uye_cikar(uygulama: tauri::AppHandle, kasa: i64, user_id: i64) -> Result<(), String> {
    kasa::uye_cikar(&uygulama.state::<Durum>(), kasa, user_id)
}

// ── Parola üreteci ─────────────────────────────────────────────────────────

/// Güçlü parola üret. Rastgelelik işletim sisteminden gelir.
#[tauri::command(async)]
fn parola_uret(uzunluk: usize, rakam: bool, simge: bool) -> String {
    kasa::parola_uret(uzunluk, rakam, simge)
}

/// Parola gücü: 0 zayıf … 3 güçlü. Arayüzdeki çubuk bunu gösterir.
#[tauri::command(async)]
fn parola_gucu(parola: String) -> u8 {
    kasa::parola_gucu(&parola)
}

// ── Otomatik güncelleme ────────────────────────────────────────────────────

/// "Güncellemeleri denetle" — kullanıcı istediğinde. Arka plan turu sessizdir;
/// burada "güncel" sonucu da arayüze bildirilir.
#[tauri::command]
async fn guncelleme_ara(uygulama: tauri::AppHandle) -> guncelleme::Durum {
    guncelleme::denetle(uygulama, true).await
}

/// İndirilmiş sürümü uygula. Başarılıysa uygulama yeniden başlar.
#[tauri::command(async)]
fn guncelleme_uygula(uygulama: tauri::AppHandle) -> Result<(), String> {
    guncelleme::uygula(&uygulama)
}

/// macOS Erişilebilirlik ayarlarını aç — kullanıcı menülerde dolaşmasın.
#[tauri::command(async)]
fn izin_ayarlarini_ac() {
    pencere::erisilebilirlik_ayarlarini_ac();
}

/// Adrese göre eşleşen kayıtlar — arayüz bunu otomatik seçim için kullanır.
#[tauri::command(async)]
fn eslesenler(uygulama: tauri::AppHandle, url: String) -> Result<Vec<Kayit>, String> {
    kasa::eslesenler(&uygulama.state::<Durum>(), &url)
}

/// Yalnızca kullanıcı adını döndürür — parola için ayrı komut yok, bilerek.
///
/// Sunucuda da ayrı uç ve ayrı izin: parolayı göremeyen bir kullanıcı da
/// kullanıcı adını kopyalayabilir. Eskiden ikisi aynı kapıdan geçtiği için
/// adresi olan her kayıtta kullanıcı adı kopyalanamıyordu.
#[tauri::command(async)]
fn kullanici_adi(uygulama: tauri::AppHandle, id: i64) -> Result<String, String> {
    kasa::kullanici_ac(&uygulama.state::<Durum>(), id)
}

/// Parolayı panoya koyar ve süre sonunda temizler.
/// Parola yine arayüze verilmez; pano işlemi Rust tarafında yapılır.
#[tauri::command(async)]
fn panoya_sifre(uygulama: tauri::AppHandle, id: i64) -> Result<u64, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    // Süre ARTIK AYARDAN. Koda gömülü bir değer, kullanıcının
    // değiştiremediği bir değerdir; 30 saniye herkese uymuyor.
    let temizleme_sn = ayarlar::oku().pano_temizleme_sn;

    let (_, sifre) = kasa::ac(&uygulama.state::<Durum>(), id)?;
    uygulama.clipboard().write_text(sifre.clone()).map_err(|e| e.to_string())?;

    // Süre sonunda temizle — ama araya başka bir şey kopyalandıysa DOKUNMA
    // 0 = "temizleme" demek; iplik açmanın anlamı yok.
    if temizleme_sn == 0 {
        return Ok(0);
    }

    let u = uygulama.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(temizleme_sn));
        if let Ok(mevcut) = u.clipboard().read_text() {
            if mevcut == sifre {
                let _ = u.clipboard().write_text(String::new());
            }
        }
        let _ = u.emit("pano-temizlendi", ());
    });

    Ok(temizleme_sn)
}

// ── Tuş gönderme ───────────────────────────────────────────────────────────

/// Metni yazar: kullanıcı adı → Tab → parola → (isteğe bağlı) Enter.
///
/// Karakterler doğrudan gönderilir (sanal tuş kodu değil), böylece Türkçe Q/F
/// veya başka bir klavye düzeni parolayı bozmaz.
fn yaz(kullanici: &str, sifre: &str, enter_bas: bool) -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut e = Enigo::new(&Settings::default()).map_err(|x| x.to_string())?;

    if !kullanici.is_empty() {
        e.text(kullanici).map_err(|x| x.to_string())?;
        e.key(Key::Tab, Direction::Click).map_err(|x| x.to_string())?;
        // Odak değişiminin oturması için bekle; hemen okumak eski alanı verir.
        std::thread::sleep(std::time::Duration::from_millis(140));

        // TAB'IN NEREYE GİTTİĞİNİ VARSAYMA — DOĞRULA.
        //
        // Buradaki sessiz kabul şuydu: "kullanıcı adından sonra Tab parola
        // alanına gider". Birçok sayfada gitmiyor — sıradaki odaklanabilir öğe
        // bir arama kutusu, bir bağlantı ya da bir düğme olabiliyor ve PAROLA
        // ORAYA yazılıyordu. Bildirilen durum tam olarak buydu: giriş yapıldı,
        // sonraki ekrandaki arama kutusuna parola gitti.
        //
        // Tespit edilemiyorsa (Windows / izin yok) eski davranışa düşülür;
        // engellemek, çalışan bir akışı hiç çalıştırmamaktan iyi değil.
        match alan::odakli_alan() {
            alan::Uygun::Bos => {}
            alan::Uygun::Bilinmiyor => {}
            alan::Uygun::Dolu => {
                return Err(
                    "Kullanıcı adı yazıldı ama sonraki alan boş değil; parola yazılmadı. \
                     Parola alanına tıklayıp tekrar deneyin."
                        .into(),
                )
            }
            alan::Uygun::AlanDegil(rol) => {
                return Err(format!(
                    "Kullanıcı adı yazıldı ama Tab bir metin alanına gitmedi ({rol}); \
                     parola yazılmadı. Parola alanına tıklayıp tekrar deneyin."
                ))
            }
        }

        /* PAROLA YALNIZCA GERÇEK PAROLA ALANINA YAZILIR.
           BİLDİRİLEN HATA: "şifreyi açık bir şekilde rastgele bir inputa
           yazıyor". Doğruydu — yukarıdaki denetim yalnız "boş bir metin
           alanı mı" diye bakıyordu. Boş bir ARAMA kutusu da boş bir metin
           alanıdır; parola oraya DÜZ METİN olarak yazılıyordu.
           `alan::parola_alani_mi()` bu iş için yazılmıştı ama hiçbir yerden
           çağrılmıyordu.
           Rol okunamıyorsa (Windows'ta AX yok) engellemiyoruz: orada
           tespit imkânı yok ve çalışan bir akışı hiç çalıştırmamak çözüm
           değil. macOS'ta izin varsa kural kesin. */
        if alan::odakli_rol().is_some() && !alan::parola_alani_mi() {
            return Err(
                "Odaktaki alan bir parola alanı değil; parola YAZILMADI. \
                 Şifrenin açıkta görünmemesi için yalnızca parola kutularına yazılıyor."
                    .into(),
            );
        }
    }
    if !sifre.is_empty() {
        e.text(sifre).map_err(|x| x.to_string())?;
    }
    if enter_bas {
        std::thread::sleep(std::time::Duration::from_millis(70));
        e.key(Key::Return, Direction::Click).map_err(|x| x.to_string())?;
    }
    Ok(())
}

/// Cihaz adı — kasa oturum kaydında "hangi bilgisayar" olarak görünür.
///
/// macOS'ta GUI uygulamalarına HOSTNAME değişkeni GEÇMEZ; bu yüzden ortam
/// değişkeni boşsa `hostname` komutuna düşülür.
fn bilgisayar_adi() -> String {
    if let Ok(a) = std::env::var("COMPUTERNAME") {
        if !a.trim().is_empty() {
            return a;
        }
    }
    if let Ok(a) = std::env::var("HOSTNAME") {
        if !a.trim().is_empty() {
            return a;
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(c) = std::process::Command::new("/bin/hostname").arg("-s").output() {
            let a = String::from_utf8_lossy(&c.stdout).trim().to_string();
            if !a.is_empty() {
                return a;
            }
        }
    }
    "Bilgisayar".into()
}

// ── Uygulama ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Durum::default())
        .manage(HedefDurum::default())
        .manage(KilitDurum::default())
        .invoke_handler(tauri::generate_handler![
            giris_yap, kod_dogrula, oturum_var, oturumu_kapat,
            kayitlar, hedef, izinler, pencereler, hedef_sec, eslesenler,
            doldur, kullanici_adi, panoya_sifre,
            guncelleme_ara, guncelleme_uygula, izin_ayarlarini_ac,
            politika_durum, politika_kur, politika_profil_kaldir,
            eklenti_kur, eklenti_kaldir, eklenti_dosyasi_var, eklenti_indir,
            eklenti_surum_durum, eklenti_tazele, kopru_durum,
            cop, cop_geri, cop_kalici_sil, gecmis, surume_don,
            cihazlar, cihaz_iptal, diger_cihazlari_kapat,
            kasalar, kasa_olustur, kasa_sil, kasa_uyeler,
            uye_ekle, uye_rol, uye_cikar,
            sistem, ayarlar_oku, ayarlar_yaz,
            kayit_ekle, kayit_guncelle, kayit_sil,
            parola_uret, parola_gucu
        ])
        .setup(|uygulama| {
            // Kayıtlı oturumu belleğe al (geçerliliği arayüz açılışında sorulur)
            if let Some(t) = kasa::jeton_oku() {
                let durum = uygulama.state::<Durum>();
                *durum.jeton.lock().unwrap() = Some(t);
            }

            // Eklenti köprüsü — tarayıcı eklentisi ayrıca giriş yapmasın diye.
            kopru::baslat(uygulama.handle().clone());
            eklenti_tazele_arkada(uygulama.handle());

            kisayol_kur(uygulama.handle())?;
            hedef_izle(uygulama.handle());
            guncelleme::zamanla(uygulama.handle());
            izin_izle(uygulama.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Bogahost Kasa baslatilamadi");
}

/// Küresel kısayol: Windows'ta Ctrl+Alt+K, macOS'ta Cmd+Alt+K.
///
/// Basıldığı anda ÖNCE hedef pencere yakalanır, SONRA kendi penceremiz açılır.
/// Sıra önemli: kendimizi önce gösterirsek öndeki pencere biz oluruz.
fn kisayol_kur(uygulama: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let degistirici = if cfg!(target_os = "macos") {
        Modifiers::SUPER | Modifiers::ALT
    } else {
        Modifiers::CONTROL | Modifiers::ALT
    };
    let kisayol = Shortcut::new(Some(degistirici), Code::KeyK);

    let u = uygulama.clone();
    uygulama.global_shortcut().on_shortcut(kisayol, move |_app, _sc, olay| {
        if olay.state() != ShortcutState::Pressed {
            return;
        }
        // Hedef, kısayola basıldığı ANDA yakalanır — kendi penceremizi hiç
        // göstermeden. Gösterseydik öndeki pencere biz olurduk.
        let h = pencere::ondeki();
        {
            let d = u.state::<HedefDurum>();
            *d.0.lock().unwrap() = h.clone();
        }

        // Ağ ve klavye işi olay işleyicisinde yapılmaz: bu işleyici ana olay
        // döngüsünde çalışıyor, orada beklemek arayüzü dondurur.
        let u2 = u.clone();
        std::thread::spawn(move || kisayol_isle(u2, h));
    })?;

    Ok(())
}

/// Kısayolun asıl işi: MÜMKÜNSE HİÇ PENCERE AÇMADAN DOLDUR.
///
/// Kullanıcıdan gelen geri bildirim buydu: "doldur demek için uygulamaya girmek
/// gerekiyor". Artık akış şu:
///   • Hedefte bir adres var ve tek/net bir eşleşme bulunduysa → doğrudan yaz.
///   • Eşleşme yok, birden fazla aday var ya da oturum yoksa → pencereyi aç,
///     kullanıcı seçsin. Yanlış kayda parola yazmaktansa sormak doğru.
fn kisayol_isle(uygulama: tauri::AppHandle, h: pencere::Hedef) {
    // HIZLI ERİŞİM PENCERESİ AÇILIR — ana uygulama DEĞİL.
    //
    // Ana pencere 620x560 ve tam bir uygulama; doldurmak için onu açmak akışı
    // bozuyordu: hedeften uzaklaşıyor, ekranı kaplıyordu. Hızlı pencere küçük,
    // çerçevesiz ve her zaman üstte — ↑/↓ ile seç, Enter ile doldur, kaybolur.
    //
    // Hızlı pencere bir sebeple yoksa ana pencereye düşülür; kullanıcı hiçbir
    // geri bildirim almadan kalmasın.
    let pencereyi_ac = |neden: &str| {
        let hedefe_gonder = |p: &tauri::WebviewWindow| {
            let _ = p.show();
            let _ = p.unminimize();
            let _ = p.set_focus();
        };
        if let Some(p) = uygulama.get_webview_window("hizli") {
            panel_yerlestir(&p);
            hedefe_gonder(&p);
            let _ = uygulama.emit("hizli-goster", ());
        } else if let Some(p) = uygulama.get_webview_window("main") {
            hedefe_gonder(&p);
        }
        let _ = uygulama.emit("hedef-degisti", h.clone());
        if !neden.is_empty() {
            let _ = uygulama.emit("kisayol-notu", neden.to_string());
        }
    };

    // Erişilebilirlik izni yoksa yazmak SESSİZCE başarısız olur; kullanıcıyı
    // "dolduruldu" diye kandırmak yerine pencereyi açıp yolu gösteriyoruz.
    if !pencere::erisilebilirlik_izni_var() {
        pencereyi_ac("izin-yok");
        return;
    }

    let Some(url) = h.url.clone() else {
        pencereyi_ac("");
        return;
    };

    let durum = uygulama.state::<Durum>();
    let eslesenler = match kasa::eslesenler(&durum, &url) {
        Ok(v) => v,
        Err(_) => {
            pencereyi_ac("");
            return;
        }
    };

    // YOL DENETİMİ: alan adı eşleşmesi tek başına yetmez. Kayıt bir giriş
    // sayfası adresi taşıyorsa, o sitenin BAŞKA sayfalarında kendiliğinden
    // doldurmak yanlış — bildirilen durumda giriş sonrası arama kutusuna
    // yazılmasının sebebi buydu.
    let eslesenler: Vec<_> = eslesenler
        .into_iter()
        .filter(|k| yol_uyar(&k.url, &url))
        .collect();

    // Tek aday yoksa karar kullanıcınındır.
    if eslesenler.len() != 1 {
        pencereyi_ac(if eslesenler.is_empty() { "eslesme-yok" } else { "coklu-eslesme" });
        return;
    }

    let kayit = &eslesenler[0];
    let (kullanici, sifre) = match kasa::doldurmak_icin_ac(&durum, kayit.id) {
        Ok(v) => v,
        Err(_) => {
            pencereyi_ac("");
            return;
        }
    };

    // Hedef zaten önde (pencere açmadık), yine de emin olalım.
    if !h.kimlik.is_empty() {
        pencere::one_getir(&h.kimlik);
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    // Kısayol yolunda da aynı denetim: yanlış alana parola yazmaktansa
    // pencereyi açıp nedenini söylemek doğru.
    let uygun = alan::odakli_alan();
    if uygun == alan::Uygun::Dolu || matches!(uygun, alan::Uygun::AlanDegil(_)) {
        pencereyi_ac("");
        let _ = uygulama.emit("kisayol-notu", format!("alan:{}", alan::aciklama(&uygun)));
        return;
    }

    // TARAYICIDA UYGULAMA YAZMAZ.
    // Sayfanın içini göremediği için hangi alanın ne olduğunu ancak dolaylı
    // işaretlerden çıkarabiliyor. Tarayıcıda bu işi eklenti yapıyor: parola
    // alanının altında kendi menüsünü açıp GÖREREK dolduruyor. Kullanıcı
    // yine de uygulamadan Doldur derse yazılır; kısayolun sessizce yazması
    // engelleniyor.
    if h.tarayici {
        pencereyi_ac("");
        let _ = uygulama.emit(
            "kisayol-notu",
            "alan:Tarayıcıda doldurmayı Bogahost Kasa eklentisi yapıyor — parola alanına tıklayınca sayfada menü açılır.".to_string(),
        );
        return;
    }

    let sonuc = yaz(&kullanici, &sifre, false);
    drop(sifre);

    match sonuc {
        Ok(()) => {
            let _ = uygulama.emit("kisayol-dolduruldu", kayit.label.clone());
            bildir(&uygulama, "Kasa", &format!("\"{}\" dolduruldu.", kayit.label));
        }
        Err(e) => {
            pencereyi_ac("");
            let _ = uygulama.emit("kisayol-notu", format!("yazma-hatasi:{e}"));
        }
    }
}

/// Eklentiyi açılışta ve sonra düzenli aralıkla sessizce tazeler.
///
/// Uygulama kendini güncelliyor ama Chrome'a "Paketlenmemiş öğe yükle" ile
/// tanıtılan eklenti güncellenmiyordu: Chrome o klasördeki DOSYALARI okur ve
/// dosyalar değişmedikçe eski sürüm sonsuza kadar çalışır. Kullanıcının bunu
/// bilmesi ve her sürümde elle indirmesi beklenemez.
///
/// TARAYICIYI KAPATMAK GEREKMİYOR: dosyalar değişince eklenti bunu köprüden
/// öğrenip `chrome.runtime.reload()` ile kendini yeniden yüklüyor.
///
/// Uygulama günlerce açık kalabiliyor; tek seferlik denetim yetmez, o yüzden
/// altı saatte bir tekrarlanıyor.
fn eklenti_tazele_arkada(uygulama: &tauri::AppHandle) {
    const ARALIK_SN: u64 = 6 * 60 * 60;
    let u = uygulama.clone();
    std::thread::spawn(move || loop {
        if let Ok(Some(surum)) = politika::eklenti_tazele() {
            let _ = u.emit("eklenti-guncellendi", surum);
        }
        std::thread::sleep(std::time::Duration::from_secs(ARALIK_SN));
    });
}

/// Erişilebilirlik iznini arka planda izler ve DEĞİŞTİĞİNDE arayüze haber verir.
///
/// NEDEN GEREKLİ: izin Sistem Ayarları'ndan veriliyor, uygulamanın dışında.
/// Yalnızca açılışta baksaydık, kullanıcı izni verdikten sonra da ekranda
/// "izin yok" yazmaya devam ederdi ve uygulamayı kapatıp açması gerekirdi.
fn izin_izle(uygulama: &tauri::AppHandle) {
    let u = uygulama.clone();
    std::thread::spawn(move || {
        let mut onceki = pencere::erisilebilirlik_izni_var();
        let _ = u.emit("izin-durumu", onceki);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let simdi = pencere::erisilebilirlik_izni_var();
            if simdi != onceki {
                onceki = simdi;
                let _ = u.emit("izin-durumu", simdi);
            }
        }
    });
}

// NOT — KALDIRILAN YOKLAMA:
// Burada giris_formu_mu() vardı: yazmadan önce Tab'a basıp sıradaki
// alanın parola alanı olup olmadığına bakıyor, sonra Shift+Tab ile geri
// dönüyordu. Amaç iyiydi ama yöntem müdahaleciydi: birçok sayfa odağı
// Shift+Tab ile geri vermiyor, bazıları da Tab'ı kendi yakalıyor. Sonuç
// "Doldur'a basıyorum, bazen hiçbir şey olmuyor" oldu.
//
// Korumayı KAYBETMİYORUZ: parolanın yanlış alana gitmesini asıl önleyen
// şey yaz() içindeki Tab SONRASI denetim — kullanıcı adı yazılıp Tab'a
// basıldıktan sonra odak metin alanı değilse ya da doluysa parola
// yazılmıyor. Kullanıcı adının yanlış kutuya düşmesi ise görünür ve
// zararsız; parolanın düşmesi değildi.


/// Adresin yol kısmı: `https://a.com/giris/?x=1#y` → `/giris`
///
/// Tam URL ayrıştırıcı eklemek yerine elle kesiliyor: yalnız yol lazım ve
/// karşılaştırma sonundaki `/`, sorgu ve çapa yok sayılarak yapılıyor.
fn yol_al(u: &str) -> String {
    let s = u.split("://").nth(1).unwrap_or(u);
    let p = match s.find('/') {
        Some(i) => &s[i..],
        None => "",
    };
    let p = p.split(['?', '#']).next().unwrap_or("");
    p.trim_end_matches('/').to_string()
}

/// Kaydın adresi hedef adresle YOL DÜZEYİNDE uyuşuyor mu?
///
/// NEDEN GEREKLİ: eşleştirme alan adı düzeyinde yapılıyor; `ornek.com/giris`
/// kaydı `ornek.com/panel/arama` ile de eşleşiyordu. Giriş yapıldıktan sonra
/// adres değişip yeni yol parçaları eklenince kayıt hâlâ "eşleşti" sayılıyor
/// ve oradaki alanlara yazılıyordu.
///
/// YOL TAM UYMALI — ALT YOLLAR ARTIK GEÇMİYOR.
/// Önceki sürüm `hedef_yol.starts_with("{kayit_yol}/")` ile alt yolları da
/// kabul ediyordu. Bildirilen durum tam olarak buydu: `/giris` kaydı giriş
/// yapıldıktan sonraki `/giris/panel`, `/giris/ayarlar` sayfalarında da
/// "eşleşti" sayılıyor, doldurma menüsü içeride de açılıyordu. Bir giriş
/// sayfasının alt yolu artık giriş sayfası değildir; sonundaki ekleri yok
/// saymanın savunulacak bir tarafı yok.
///
/// Kayıtta yol yoksa (yalnız alan adı girilmişse) alan adı eşleşmesi yeterli
/// sayılır — kullanıcı orayı bilerek geniş bırakmıştır.
pub(crate) fn yol_uyar(kayit_url: &Option<String>, hedef_url: &str) -> bool {
    let Some(k) = kayit_url else { return true };
    let kayit_yol = yol_al(k);
    if kayit_yol.is_empty() {
        return true;
    }
    yol_al(hedef_url) == kayit_yol
}

/// Kısa sistem bildirimi — pencere açmadığımız için tek geri bildirim bu.
fn bildir(uygulama: &tauri::AppHandle, baslik: &str, govde: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = uygulama.notification().builder().title(baslik).body(govde).show();
}

/// Öndeki pencereyi arka planda izler ve KENDİMİZ DIŞINDAKİ son pencereyi
/// hedef olarak saklar.
///
/// NEDEN GEREKLİ: kullanıcının hedefi elle seçmesi ya da her seferinde kısayola
/// basması işi uzatıyordu. Profesyonel parola yöneticileri de böyle çalışır:
/// siz uygulamaya geçtiğinizde hedef ZATEN belirlenmiştir, çünkü ondan önceki
/// pencere biliniyordur.
///
/// KENDİ PENCEREMİZ NEDEN ATLANIYOR: kullanıcı Kasa'ya tıkladığı anda öndeki
/// pencere biz oluruz. O anki değeri yazsaydık hedef her seferinde "Bogahost
/// Kasa" olurdu — yani hiçbir zaman doğru olmazdı.
///
/// MALİYET: macOS'ta her tur bir `osascript` çağırıyor. Bu yüzden kendi
/// penceremiz odaktayken tur ATLANIYOR — kullanıcı zaten Kasa'ya bakıyorsa
/// hedefin değişmesi mümkün değil ve boşuna süreç açmanın anlamı yok.
fn hedef_izle(uygulama: &tauri::AppHandle) {
    const ARALIK_MS: u64 = 1500;
    let u = uygulama.clone();

    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(ARALIK_MS));

        // Kendi penceremiz odaktaysa tur atla.
        let bizdeyiz = u
            .get_webview_window("main")
            .and_then(|p| p.is_focused().ok())
            .unwrap_or(false);
        if bizdeyiz {
            continue;
        }

        let h = pencere::ondeki();
        if h.program.is_empty() || h.program == "Bogahost Kasa" {
            continue;
        }

        // Aynı hedefe tekrar tekrar olay göndermeyelim; arayüz boşuna
        // eşleşme sorgusu yapmasın diye kimlik VE adres birlikte kıyaslanıyor
        // (aynı tarayıcıda sekme değişince adres değişir, kimlik değişmez).
        let durum = u.state::<HedefDurum>();
        let degisti = {
            let mevcut = durum.0.lock().unwrap();
            mevcut.kimlik != h.kimlik || mevcut.url != h.url
        };
        if !degisti {
            continue;
        }

        *durum.0.lock().unwrap() = h.clone();
        let _ = u.emit("hedef-degisti", h.clone());

        // EŞLEŞİNCE PANEL KENDİLİĞİNDEN EKRANA GELİR.
        //
        // Kullanıcının isteği: "eşleşme yapınca bilgisayar ekranına otomatik
        // düşmeli". Kısayola basmayı beklemek, doldurmanın önündeki asıl
        // engeldi — kimse her seferinde tuş kombinasyonu hatırlamak zorunda
        // kalmamalı.
        //
        // ODAK ÇALINMIYOR: panel `show()` ile geliyor ama `set_focus()`
        // çağrılmıyor. Kullanıcı yazmaya devam edebilsin diye; panele geçmek
        // isterse tıklar ya da kısayola basar. Odağı almak, kullanıcının o an
        // yazdığı şeyi bölmek demekti.
        panel_belirt(&u, &h);
    });
}

/// Paneli ekranın SAĞ ALTINA yerleştirir.
///
/// NEDEN ORTA DEĞİL: ortada açılan bir pencere tam olarak baktığınız yeri
/// kapatıyor — giriş formunun üstüne oturuyor ve rahatsız ediyor. Sağ alt köşe
/// göz ucuyla görülüyor ama çalışılan alanı örtmüyor; bildirim panellerinin
/// orada olmasının sebebi de bu.
fn panel_yerlestir(panel: &tauri::WebviewWindow) {
    use tauri::{PhysicalPosition, PhysicalSize};

    let Ok(Some(ekran)) = panel.current_monitor() else { return };
    let PhysicalSize { width: ew, height: eh } = *ekran.size();
    let Ok(PhysicalSize { width: pw, height: ph }) = panel.outer_size() else { return };
    let konum = ekran.position();
    let olcek = ekran.scale_factor();

    // Kenar boşluğu ekran ölçeğine göre; Retina'da 24 mantıksal piksel.
    let bosluk = (24.0 * olcek) as i32;
    let x = konum.x + ew as i32 - pw as i32 - bosluk;
    let y = konum.y + eh as i32 - ph as i32 - bosluk * 2;
    let _ = panel.set_position(PhysicalPosition::new(x, y));
}

/// Doldurulduktan sonra o SİTE için panel bir süre kendiliğinden açılmasın.
///
/// ÖNCE ADRESE BAKIYORDU, YETMEDİ. Bildirilen durum: "giriş yapınca da hâlâ
/// kasa görünmeye devam ediyor". Sebebi şuydu — giriş yapıldıktan sonra
/// adres değişiyor (`/giris` → `/panel`), bizim sakladığımız adres artık
/// tutmuyor ve panel yeniden açılıyordu.
///
/// Artık ALAN ADI saklanıyor: bir siteye giriş yaptıysanız o sitede işiniz
/// bitmiştir. Süre dolunca ya da başka bir siteye geçince panel yine
/// çalışıyor; kullanıcı isterse kısayolla her zaman açabiliyor.
static SON_DOLDURULAN: Mutex<Vec<(String, std::time::Instant)>> = Mutex::new(Vec::new());

/// Doldurulan sitede panel ne kadar sessiz kalsın — AYARDAN.

/// Adresin alan adı — `https://a.com/x?y` → `a.com`
fn alan_adi(u: &str) -> String {
    let s = u.split("://").nth(1).unwrap_or(u);
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    host.split('@').last().unwrap_or(host)
        .split(':').next().unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase()
}

/// Bu adres az önce dolduruldu mu? Süresi geçmiş kayıtlar da burada temizlenir.
pub(crate) fn az_once_dolduruldu(url: &str) -> bool {
    let alan = alan_adi(url);
    if alan.is_empty() {
        return false;
    }
    let mut liste = SON_DOLDURULAN.lock().unwrap();
    let sure = std::time::Duration::from_secs(ayarlar::oku().sessizlik_dk.max(1) * 60);
    liste.retain(|(_, t)| t.elapsed() < sure);
    liste.iter().any(|(a, _)| *a == alan)
}

pub(crate) fn dolduruldu_isaretle(url: &str) {
    let alan = alan_adi(url);
    if alan.is_empty() {
        return;
    }
    let mut liste = SON_DOLDURULAN.lock().unwrap();
    liste.retain(|(a, _)| *a != alan);
    liste.push((alan, std::time::Instant::now()));
}

/// Hedefte eşleşen kayıt varsa hızlı paneli gösterir, yoksa gizler.
///
/// Sunucuya sorulan tek şey eşleşme listesi; parola burada hiç görünmüyor.
/// Ağ işi zaten arka plan ipliğinde, kullanıcı arayüzünü bekletmiyor.
fn panel_belirt(uygulama: &tauri::AppHandle, h: &pencere::Hedef) {
    let Some(panel) = uygulama.get_webview_window("hizli") else { return };

    let acik = panel.is_visible().unwrap_or(false);

    // Panel kullanıcının elindeyse (odakta) hiç karışma.
    if acik && panel.is_focused().unwrap_or(false) {
        return;
    }

    // Adresi okunamayan hedefte (masaüstü programı) eşleştirecek bir şey yok.
    // Panel açıksa KAPATILIR: başka bir programa geçildiğinde ekranda asılı
    // kalması, yardım değil rahatsızlık olur.
    let Some(url) = h.url.clone() else {
        if acik {
            let _ = panel.hide();
        }
        return;
    };

    // Yazma izni yoksa panel göstermek boşuna umut olur.
    if !pencere::erisilebilirlik_izni_var() {
        return;
    }

    // Kullanıcı panelin kendiliğinden açılmasını kapatmış olabilir.
    if !ayarlar::oku().panel_kendiliginden {
        return;
    }

    // Bu sitede az önce dolduruldu — panel ısrar etmesin.
    if az_once_dolduruldu(&url) {
        if acik {
            let _ = panel.hide();
        }
        return;
    }

    let durum = uygulama.state::<Durum>();
    let Ok(eslesenler) = kasa::eslesenler(&durum, &url) else { return };
    let uyanlar: Vec<_> = eslesenler
        .into_iter()
        .filter(|k| yol_uyar(&k.url, &url))
        .collect();

    if uyanlar.is_empty() {
        // Bu adres için kayıt yok — açıksa kapat, kapalıysa açma.
        if acik {
            let _ = panel.hide();
        }
        return;
    }

    if !acik {
        panel_yerlestir(&panel);
        let _ = panel.show();
    }
    let _ = uygulama.emit("hizli-goster", ());
}