//! Yerel köprü — Chrome eklentisi bilgisayardaki Kasa oturumunu kullanır.
//!
//! NEDEN VAR: eklentinin kendi jetonu vardı. Personel DCIM'e girip "Eklentiyi
//! Bağla" demek, oradan çıkan jetonu eklentiye taşımak zorundaydı. Oysa aynı
//! bilgisayarda zaten giriş yapılmış bir Kasa uygulaması duruyor. İki ayrı
//! kimlik tutmanın karşılığı yok: ikinci bir sır daha üretiliyor, ikinci bir
//! yerde saklanıyor, hesap kapanınca ikisinin de ayrı ayrı düşmesi gerekiyor.
//!
//! Artık tek kimlik var. Eklenti 127.0.0.1'deki bu köprüye soruyor, köprü
//! uygulamanın oturumuyla sunucuya gidiyor. Kasa'da oturum kapanınca eklenti
//! de aynı saniye içinde boş kalır — ayrıca bir şey yapmak gerekmez.
//!
//! GÜVENLİK SINIRI:
//!   • Dinleme YALNIZCA 127.0.0.1 — ağdan erişilemez.
//!   • `Origin` başlığı tam olarak kendi eklentimizin kimliği olmalı.
//!     Tarayıcı bu başlığı sayfa betiğine yazdırmaz; bir web sayfası kendini
//!     `chrome-extension://…` diye tanıtamaz. Başka bir eklentinin kimliği de
//!     farklıdır.
//!   • Parola YALNIZCA soran sayfanın adresiyle eşleşen kayıt için verilir.
//!     Uygulamanın kendi doldurma yolundaki denetimin aynısı.
//!
//! Parola diske yazılmaz, günlüğe düşmez; yanıtta bir kez geçer ve eklenti onu
//! doğrudan alana yazar.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};

use crate::kasa::{self, Durum};
use tauri::{Emitter, Manager};

/// Eklentinin denediği aralık. Tek bir sabit port, o port başkasındayken
/// köprüyü tamamen kullanılamaz yapardı; eklenti sırayla deneyip bulur.
pub const PORTLAR: std::ops::RangeInclusive<u16> = 17321..=17330;

/// İstek gövdesi için üst sınır — köprüye gelen JSON birkaç yüz bayt.
const EN_BUYUK_GOVDE: usize = 8 * 1024;

/// Açılan port. Arayüz "Tarayıcı bağlantısı" satırında bunu gösteriyor;
/// köprü hiç açılamadıysa `None` kalır ve satır kırmızı yanar.
static PORT: std::sync::Mutex<Option<u16>> = std::sync::Mutex::new(None);

/// UYGULAMADAN EKLENTİYE İŞ KUYRUĞU.
///
/// Panel tarayıcı hedefinde "doldurmayı eklenti yapıyor" diye bir metin
/// gösteriyordu. Bu bir cevap değil, bahane: kullanıcı paneli görüyor,
/// kaydı görüyor, ama tıklayınca bir şey olmuyordu.
///
/// Artık panel tıklandığında iş buraya bırakılıyor; eklenti köprüyü zaten
/// dinliyor ve saniyeler içinde alıp sayfada dolduruyor. Parola bu kuyruktan
/// GEÇMİYOR — yalnız hangi kaydın hangi adreste doldurulacağı yazıyor;
/// eklenti sonra her zamanki gibi `/doldur` diyerek kendi alıyor.
static OLAYLAR: std::sync::Mutex<Vec<serde_json::Value>> = std::sync::Mutex::new(Vec::new());

/// Uzun yoklamanın en fazla bekleyeceği süre.
///
/// Tarayıcı ve işletim sistemi boşta duran bağlantıları kesiyor; ayrıca
/// eklentinin servis işçisi uzun süre yanıt beklerken uyutulabiliyor. 25
/// saniye, "anında gelsin" ile "bağlantı kopmasın" arasında duruyor.
const OLAY_BEKLEME_SN: u64 = 25;

/// Eklenti son zamanlarda köprüye uğradı mı?
///
/// Eklenti dakikada bir durum soruyor ve olay ucunu sürekli açık tutuyor.
/// İki dakikadır ses yoksa kurulu değil ya da kapalı demektir — bu durumda
/// panelden "iletildi" demek yalan olurdu, iş kuyrukta çürürdü.
static SON_TEMAS: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

pub fn eklenti_bagli() -> bool {
    SON_TEMAS
        .lock()
        .unwrap()
        .map(|t| t.elapsed() < std::time::Duration::from_secs(150))
        .unwrap_or(false)
}

fn temas_kaydet() {
    *SON_TEMAS.lock().unwrap() = Some(std::time::Instant::now());
}

pub fn olay_ekle(olay: serde_json::Value) {
    let mut k = OLAYLAR.lock().unwrap();
    // Kuyruk birikmesin: eklenti kapalıyken tıklanan her şey sonsuza kadar
    // beklerse, açıldığı an arka arkaya doldurmaya kalkar.
    if k.len() > 8 {
        k.remove(0);
    }
    k.push(olay);
}

fn olay_al() -> Option<serde_json::Value> {
    OLAYLAR.lock().unwrap().pop()
}

/// Köprünün durumu — arayüz için.
pub fn durum() -> serde_json::Value {
    match *PORT.lock().unwrap() {
        Some(p) => serde_json::json!({
            "calisiyor": true, "port": p, "eklenti_bagli": eklenti_bagli(),
        }),
        None => serde_json::json!({"calisiyor": false, "port": 0, "eklenti_bagli": false}),
    }
}

/// Köprüyü ayrı bir iplikte başlatır. Port bulunamazsa sessizce vazgeçer:
/// köprü olmadan da uygulama tam çalışır, yalnız eklenti bağlanamaz.
pub fn baslat(uygulama: tauri::AppHandle) {
    std::thread::spawn(move || {
        let Some((dinleyici, port)) = ac() else {
            eprintln!("kasa: yerel köprü için boş port bulunamadı");
            return;
        };
        *PORT.lock().unwrap() = Some(port);
        for baglanti in dinleyici.incoming() {
            let Ok(akis) = baglanti else { continue };
            let u = uygulama.clone();
            // Her istek kendi ipliğinde: sunucuya gidiş 20 sn sürebiliyor,
            // bu sırada başka bir sekmenin sorgusu beklememeli.
            std::thread::spawn(move || {
                let _ = isle(u, akis);
            });
        }
    });
}

fn ac() -> Option<(TcpListener, u16)> {
    for p in PORTLAR {
        if let Ok(d) = TcpListener::bind((Ipv4Addr::LOCALHOST, p)) {
            return Some((d, p));
        }
    }
    None
}

struct Istek {
    yontem: String,
    yol: String,
    origin: String,
    /// Eklentinin kendi imzası — Chrome bazı isteklerde `Origin` göndermiyor.
    imza: String,
    govde: String,
}

fn oku(akis: &TcpStream) -> Option<Istek> {
    let mut okuyucu = BufReader::new(akis);

    let mut satir = String::new();
    okuyucu.read_line(&mut satir).ok()?;
    let mut parca = satir.split_whitespace();
    let yontem = parca.next()?.to_string();
    let yol = parca.next()?.to_string();

    let mut origin = String::new();
    let mut imza = String::new();
    let mut uzunluk = 0usize;
    loop {
        let mut b = String::new();
        if okuyucu.read_line(&mut b).ok()? == 0 {
            break;
        }
        let b = b.trim_end();
        if b.is_empty() {
            break;
        }
        let (ad, deger) = match b.split_once(':') {
            Some((a, d)) => (a.trim().to_ascii_lowercase(), d.trim().to_string()),
            None => continue,
        };
        match ad.as_str() {
            "origin" => origin = deger,
            "x-kasa" => imza = deger,
            "content-length" => uzunluk = deger.parse().unwrap_or(0),
            _ => {}
        }
    }

    let mut govde = String::new();
    if uzunluk > 0 {
        let n = uzunluk.min(EN_BUYUK_GOVDE);
        let mut tampon = vec![0u8; n];
        okuyucu.read_exact(&mut tampon).ok()?;
        govde = String::from_utf8_lossy(&tampon).into_owned();
    }

    Some(Istek { yontem, yol, origin, imza, govde })
}

fn yanit(akis: &mut TcpStream, kod: u16, durum: &str, govde: &str) {
    let izinli = format!("chrome-extension://{}", crate::politika::EKLENTI_ID);
    let ham = format!(
        "HTTP/1.1 {kod} {durum}\r\n\
         Content-Type: application/json; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: {izinli}\r\n\
         Access-Control-Allow-Headers: content-type, x-kasa\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Max-Age: 600\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n{govde}",
        govde.len()
    );
    let _ = akis.write_all(ham.as_bytes());
    let _ = akis.flush();
}

fn isle(uygulama: tauri::AppHandle, mut akis: TcpStream) -> std::io::Result<()> {
    akis.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    akis.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;

    let Some(istek) = oku(&akis) else {
        yanit(&mut akis, 400, "Bad Request", r#"{"ok":false,"hata":"istek okunamadi"}"#);
        return Ok(());
    };

    // KİMLİK DENETİMİ — İKİ YOL, İKİSİ DE SAYFADAN TAKLİT EDİLEMEZ.
    //
    // 1) `Origin` tam olarak bizim eklentimiz. Bu başlığı tarayıcı yazar,
    //    sayfa betiği değiştiremez.
    // 2) Chrome, host izni verilmiş adreslere giden bazı eklenti isteklerinde
    //    `Origin` GÖNDERMİYOR. O durumda eklentinin kendi kimliğini yazdığı
    //    `X-Kasa` başlığına bakılıyor. Bir web sayfası bu başlığı ancak ön
    //    kontrollü (preflight) istekle ekleyebilir; ön kontrol yanıtımızdaki
    //    izin yalnız kendi eklentimize verildiği için tarayıcı o isteği
    //    başlatmaz ve yanıtı okutmaz.
    let izinli = format!("chrome-extension://{}", crate::politika::EKLENTI_ID);
    let bizden = istek.origin == izinli
        || (istek.origin.is_empty() && istek.imza == crate::politika::EKLENTI_ID);
    if !bizden {
        yanit(&mut akis, 403, "Forbidden", r#"{"ok":false,"hata":"yetkisiz kaynak"}"#);
        return Ok(());
    }

    // Buraya gelen istek kimlik denetimini geçti; yani eklenti ayakta.
    temas_kaydet();

    if istek.yontem == "OPTIONS" {
        yanit(&mut akis, 204, "No Content", "");
        return Ok(());
    }

    let (yol, sorgu) = match istek.yol.split_once('?') {
        Some((y, s)) => (y, s),
        None => (istek.yol.as_str(), ""),
    };

    let cevap = match yol {
        "/durum" => durum_cevabi(&uygulama),
        "/olay" => olay_cevabi(),
        "/eslesenler" => eslesenler_cevabi(&uygulama, &sorgu_al(sorgu, "url")),
        "/doldur" => doldur_cevabi(&uygulama, &istek.govde),
        _ => serde_json::json!({"ok": false, "hata": "bilinmeyen uc"}),
    };

    yanit(&mut akis, 200, "OK", &cevap.to_string());
    Ok(())
}

/// UZUN YOKLAMA: iş çıkana kadar bekle, çıkmazsa boş dön.
///
/// Eklenti bu ucu sürekli açık tutuyor. Sıradan bir yoklama (her dakika sor)
/// kullanıcıyı bir dakikaya kadar bekletirdi; tıkladıktan sonra bir dakika
/// beklemek "çalışmıyor" demektir.
fn olay_cevabi() -> serde_json::Value {
    let bitis = std::time::Instant::now() + std::time::Duration::from_secs(OLAY_BEKLEME_SN);
    loop {
        if let Some(o) = olay_al() {
            return serde_json::json!({ "ok": true, "olay": o });
        }
        if std::time::Instant::now() >= bitis {
            return serde_json::json!({ "ok": true, "olay": serde_json::Value::Null });
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

/// `url=https%3A%2F%2F…` → çözülmüş değer.
fn sorgu_al(sorgu: &str, anahtar: &str) -> String {
    for parca in sorgu.split('&') {
        if let Some((a, d)) = parca.split_once('=') {
            if a == anahtar {
                return yuzde_coz(d);
            }
        }
    }
    String::new()
}

fn yuzde_coz(s: &str) -> String {
    let bayt = s.as_bytes();
    let mut cikti = Vec::with_capacity(bayt.len());
    let mut i = 0;
    while i < bayt.len() {
        match bayt[i] {
            b'%' if i + 2 < bayt.len() => {
                let onaltilik = std::str::from_utf8(&bayt[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(onaltilik, 16) {
                    Ok(b) => {
                        cikti.push(b);
                        i += 3;
                    }
                    Err(_) => {
                        cikti.push(bayt[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                cikti.push(b' ');
                i += 1;
            }
            b => {
                cikti.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&cikti).into_owned()
}

/// Eklentinin dakikada bir sorduğu uç.
///
/// İki sürüm numarası taşıyor ve eklentinin KENDİNİ YENİLEMESİ bunlara bağlı:
///   • `eklenti` — İndirilenler'deki paketin sürümü. Eklenti paketlenmemiş
///     kurulduysa Chrome dosyaları oradan okur; kendi sürümünden farklıysa
///     dosyalar değişmiş demektir ve eklenti `chrome.runtime.reload()` ile
///     kendini yeniden yükler. Tarayıcıyı kapatmaya gerek kalmaz.
///   • `yayin` — sunucuda duran sürüm. CRX olarak kurulmuş eklenti bunu
///     görünce Chrome'dan güncelleme denetimi istiyor.
fn durum_cevabi(uygulama: &tauri::AppHandle) -> serde_json::Value {
    let acik = uygulama.state::<Durum>().jeton.lock().unwrap().is_some();
    serde_json::json!({
        "ok": true,
        "oturum": acik,
        "surum": uygulama.package_info().version.to_string(),
        "eklenti": crate::politika::eklenti_yerel_surum(),
        "yayin": crate::politika::eklenti_yayin_surum(),
    })
}

/// Bu adrese uyan kayıtlar — PAROLA YOK. Eklenti bunu yalnızca alanın yanındaki
/// menüyü çizmek için kullanır.
fn eslesenler_cevabi(uygulama: &tauri::AppHandle, url: &str) -> serde_json::Value {
    if url.is_empty() {
        return serde_json::json!({"ok": false, "hata": "adres yok"});
    }
    let durum = uygulama.state::<Durum>();
    if durum.jeton.lock().unwrap().is_none() {
        return serde_json::json!({"ok": false, "hata": "oturum yok", "oturum": false});
    }
    match kasa::eslesenler(&durum, url) {
        Ok(liste) => {
            let uyanlar: Vec<_> = liste
                .into_iter()
                .filter(|k| crate::yol_uyar(&k.url, url))
                .map(|k| {
                    serde_json::json!({
                        "id": k.id,
                        "etiket": k.label,
                        "kullanici": k.username,
                        "alan": k.domain,
                    })
                })
                .collect();
            serde_json::json!({"ok": true, "oturum": true, "kayitlar": uyanlar})
        }
        Err(e) => serde_json::json!({"ok": false, "hata": e}),
    }
}

/// Parolayı eklentiye verir — YALNIZCA soran sayfa kayda gerçekten uyuyorsa.
///
/// Kimlik denetimini geçmiş olmak "her kaydı isteyebilir" demek değil. Eklenti
/// hangi sayfada olduğunu bildiriyor; o sayfayla eşleşmeyen bir kaydın parolası
/// verilmiyor. Uygulamanın kendi `doldur` komutundaki kural bire bir aynı.
fn doldur_cevabi(uygulama: &tauri::AppHandle, govde: &str) -> serde_json::Value {
    let istek: serde_json::Value = match serde_json::from_str(govde) {
        Ok(v) => v,
        Err(_) => return serde_json::json!({"ok": false, "hata": "gecersiz istek"}),
    };
    let id = istek["id"].as_i64().unwrap_or(0);
    let url = istek["url"].as_str().unwrap_or("").to_string();
    if id == 0 || url.is_empty() {
        return serde_json::json!({"ok": false, "hata": "eksik istek"});
    }

    let durum = uygulama.state::<Durum>();
    if durum.jeton.lock().unwrap().is_none() {
        return serde_json::json!({"ok": false, "hata": "oturum yok", "oturum": false});
    }

    let uygun = kasa::eslesenler(&durum, &url)
        .map(|l| {
            l.into_iter()
                .any(|k| k.id == id && crate::yol_uyar(&k.url, &url))
        })
        .unwrap_or(false);
    if !uygun {
        return serde_json::json!({"ok": false, "hata": "kayit bu adrese ait degil"});
    }

    match kasa::doldurmak_icin_ac(&durum, id) {
        Ok((kullanici, parola)) => {
            // Eklenti doldurduysa masaüstü paneli o sitede ısrar etmesin.
            crate::dolduruldu_isaretle(&url);
            let _ = uygulama.emit("kisayol-dolduruldu", "eklenti");
            serde_json::json!({"ok": true, "kullanici": kullanici, "parola": parola})
        }
        Err(e) => serde_json::json!({"ok": false, "hata": e}),
    }
}
