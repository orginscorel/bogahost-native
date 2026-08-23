//! Otomatik güncelleme.
//!
//! Diğer uygulamalarla aynı altyapı (`tauri-plugin-updater`, aynı imza anahtarı,
//! `native.bogahost.com/updates/kasa/...`), ama akış Kasa'ya göre sadeleştirildi:
//! tepsi menüsü, sayfa-meşgul bayrağı ve canlı URL takibi burada yok.
//!
//! İKİ TUZAK, İKİSİ DE BİLEREK BÖYLE ÇÖZÜLDÜ:
//!
//! 1. WINDOWS'TA "SESSİZCE KUR, SONRA SOR" MÜMKÜN DEĞİL.
//!    `Update::install`, kurulum programını çalıştırıp süreci `exit(0)` ile
//!    öldürür. Bu yüzden arka planda yalnızca İNDİRİLİR; kurulum, kullanıcı
//!    "Şimdi güncelle" dediği an yapılır. Aksi hâlde kullanıcı parola yazarken
//!    uygulama aniden kapanırdı.
//!
//! 2. İMZASIZ macOS PAKETİ KENDİNİ DEĞİŞTİREMEZ.
//!    Çalışan uygulama kendi bundle'ını yazamadığında kurulum kalıcı olarak
//!    başarısızdır; tekrar denemek işe yaramaz. O durumda kullanıcıya indirme
//!    sayfası açılır.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// İndirme sayfası — otomatik kurulum yapılamadığında elle yol.
const INDIRME_SAYFASI: &str = "https://native.bogahost.com/";

/// Aynı anda iki denetim/indirme çalışmasın.
static SURUYOR: AtomicBool = AtomicBool::new(false);
/// Otomatik kurulum kalıcı olarak başarısız (imzasız macOS) → elle indirme göster.
static ELLE_GEREKLI: AtomicBool = AtomicBool::new(false);

/// İndirilmiş, kullanıcının onayını bekleyen sürüm.
struct Bekleyen {
    surum: String,
    /// YALNIZCA Windows'ta dolu — kurulum onaydan sonra yapılacağı için
    /// indirilen baytlar burada tutulur (bkz. dosya başındaki 1. tuzak).
    kurulum: Option<(tauri_plugin_updater::Update, Vec<u8>)>,
}

static BEKLEYEN: Mutex<Option<Bekleyen>> = Mutex::new(None);

/// Arayüze gönderilen durum.
#[derive(serde::Serialize, Clone)]
pub struct Durum {
    pub var: bool,
    pub surum: Option<String>,
    /// true ise otomatik kurulum yapılamıyor; "İndirme sayfasını aç" gösterilmeli.
    pub elle: bool,
}

/// Açılışta ve periyodik olarak denetle.
///
/// Açılışta 6 saniye beklenir: kullanıcı ilk saniyelerde giriş yapıyor, ağ
/// isteğini o anın üstüne bindirmenin anlamı yok.
pub fn zamanla(uygulama: &tauri::AppHandle) {
    const ILK_GECIKME_SN: u64 = 6;
    const ARALIK_SN: u64 = 6 * 60 * 60; // 6 saat

    let u = uygulama.clone();
    // Kendi ipliğinde uyuyup denetimi async çalışma zamanında yürütür. Yalnızca
    // periyodik uyku için `tokio`yu doğrudan bağımlılığa eklemenin anlamı yok.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(ILK_GECIKME_SN));
        loop {
            tauri::async_runtime::block_on(denetle(u.clone(), false));
            std::thread::sleep(std::time::Duration::from_secs(ARALIK_SN));
        }
    });
}

/// Sürüm denetle; varsa indir ve arayüze haber ver.
///
/// `elle_istendi`: kullanıcı "Güncellemeleri denetle" dediyse true — o zaman
/// "güncel" sonucu da bildirilir. Arka plan turunda sessiz kalınır.
pub async fn denetle(uygulama: tauri::AppHandle, elle_istendi: bool) -> Durum {
    let bos = Durum { var: false, surum: None, elle: false };

    // Zaten indirilmiş bir sürüm bekliyorsa tekrar indirme.
    if let Some(b) = BEKLEYEN.lock().unwrap().as_ref() {
        return Durum { var: true, surum: Some(b.surum.clone()), elle: ELLE_GEREKLI.load(Ordering::SeqCst) };
    }
    if SURUYOR.swap(true, Ordering::SeqCst) {
        return bos;
    }

    let sonuc = async {
        let updater = match uygulama.updater() {
            Ok(u) => u,
            Err(_) => return bos.clone(), // imzasız build: updater yapılandırılmamış
        };
        let guncelleme = match updater.check().await {
            Ok(Some(g)) => g,
            // Güncel ya da ağ hatası: arka planda sessiz, elle istendiyse arayüz bilsin.
            _ => return bos.clone(),
        };
        let surum = guncelleme.version.clone();

        // İNDİR — her platformda aynı. Ağ hatasında sessizce bırakılır, bir
        // sonraki turda tekrar denenir.
        let baytlar = match guncelleme.download(|_, _| {}, || {}).await {
            Ok(b) => b,
            Err(_) => return bos.clone(),
        };

        #[cfg(target_os = "windows")]
        {
            // Kurulum SÜRECİ ÖLDÜRÜR; onaya kadar bekletiyoruz.
            *BEKLEYEN.lock().unwrap() = Some(Bekleyen {
                surum: surum.clone(),
                kurulum: Some((guncelleme, baytlar)),
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            // macOS: kurulum süreç çalışırken tamamlanır; onay yalnızca yeniden
            // başlatmayı tetikler. Başarısızsa bu kalıcıdır (imzasız paket).
            match guncelleme.install(&baytlar) {
                Ok(()) => {
                    *BEKLEYEN.lock().unwrap() = Some(Bekleyen { surum: surum.clone(), kurulum: None });
                }
                Err(_) => {
                    ELLE_GEREKLI.store(true, Ordering::SeqCst);
                    *BEKLEYEN.lock().unwrap() = Some(Bekleyen { surum: surum.clone(), kurulum: None });
                }
            }
        }

        Durum { var: true, surum: Some(surum), elle: ELLE_GEREKLI.load(Ordering::SeqCst) }
    }
    .await;

    SURUYOR.store(false, Ordering::SeqCst);

    if sonuc.var || elle_istendi {
        let _ = uygulama.emit("guncelleme-durumu", sonuc.clone());
    }
    sonuc
}

/// Bekleyen sürümü uygula. Geri DÖNMEZ (uygulama yeniden başlar) — hata dışında.
pub fn uygula(uygulama: &tauri::AppHandle) -> Result<(), String> {
    if ELLE_GEREKLI.load(Ordering::SeqCst) {
        indirme_sayfasini_ac();
        return Ok(());
    }

    let bekleyen = BEKLEYEN.lock().unwrap().take();
    let Some(b) = bekleyen else {
        return Err("Uygulanacak güncelleme yok.".into());
    };

    #[cfg(target_os = "windows")]
    {
        let Some((guncelleme, baytlar)) = b.kurulum else {
            return Err("İndirilmiş kurulum bulunamadı.".into());
        };
        // Bu çağrı kurulumu başlatır ve süreci öldürür; sonrası çalışmaz.
        if let Err(e) = guncelleme.install(&baytlar) {
            ELLE_GEREKLI.store(true, Ordering::SeqCst);
            return Err(format!("Kurulum başarısız: {e}. İndirme sayfasından kurabilirsiniz."));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS: paket zaten değiştirildi, yalnız yeniden başlatılır.
        // restart() geri DÖNMEZ.
        drop(b);
        uygulama.restart()
    }
}

/// Otomatik kurulum yapılamıyorsa kullanıcıyı indirme sayfasına götür.
///
/// Ayrı bir "opener" eklentisi eklemek yerine işletim sisteminin kendi komutu
/// kullanılıyor; tek bir adres açmak için bağımlılık büyütmenin anlamı yok.
fn indirme_sayfasini_ac() {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(INDIRME_SAYFASI).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", INDIRME_SAYFASI])
        .spawn();
}
