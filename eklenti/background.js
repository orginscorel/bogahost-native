/* Bogahost Kasa — arka plan (service worker).

   ARTIK KENDİ JETONU YOK. Eskiden personel DCIM'e girip "Eklentiyi Bağla"
   diyor, çıkan jetonu eklentiye taşıyordu. Oysa aynı bilgisayarda zaten giriş
   yapılmış bir Kasa uygulaması var. İkinci bir kimlik tutmanın karşılığı yok:
   ikinci bir sır üretiliyor, ikinci bir yerde saklanıyor, hesap kapanınca
   ikisinin ayrı ayrı düşmesi gerekiyor.

   Bu sürümde eklenti 127.0.0.1'deki Kasa uygulamasına soruyor; sunucuya
   uygulama gidiyor. Kasa'dan çıkış yapılınca eklenti de aynı anda boş kalır.

   Şifre hiçbir yerde saklanmıyor: "doldur" anında bir kez geliyor, alana
   yazılıyor ve bırakılıyor. */

const KIMLIK = chrome.runtime.id;
/* Uygulama bu aralıkta boş bulduğu ilk portu açar. Tek sabit port, o port
   başkasındayken eklentiyi tamamen çalışmaz yapardı. */
const PORTLAR = [17321, 17322, 17323, 17324, 17325, 17326, 17327, 17328, 17329, 17330];

let PORT = null;

async function portOku() {
  if (PORT !== null) return PORT;
  try {
    const s = await chrome.storage.session.get('port');
    if (s && s.port) PORT = s.port;
  } catch (e) {}
  return PORT;
}
async function portYaz(p) {
  PORT = p;
  try { await chrome.storage.session.set({ port: p }); } catch (e) {}
}

function adres(p, yol) { return 'http://127.0.0.1:' + p + yol; }

async function dene(p) {
  try {
    const c = new AbortController();
    const zaman = setTimeout(() => c.abort(), 1200);
    const y = await fetch(adres(p, '/durum'), {
      headers: { 'X-Kasa': KIMLIK }, cache: 'no-store', signal: c.signal,
    });
    clearTimeout(zaman);
    if (!y.ok) return null;
    const d = await y.json();
    return d && d.ok ? d : null;
  } catch (e) { return null; }
}

/* Portu bulur. Önce bilinen portu dener (uygulama yeniden başlamadıysa hep
   aynıdır), tutmazsa aralığı tarar. */
async function kopru() {
  const bilinen = await portOku();
  if (bilinen !== null) {
    const d = await dene(bilinen);
    if (d) return { port: bilinen, durum: d };
  }
  for (const p of PORTLAR) {
    if (p === bilinen) continue;
    const d = await dene(p);
    if (d) { await portYaz(p); return { port: p, durum: d }; }
  }
  PORT = null;
  try { await chrome.storage.session.remove('port'); } catch (e) {}
  return null;
}

async function cagir(yol, secenek) {
  const k = await kopru();
  if (!k) return { ok: false, hata: 'kasa-kapali' };
  try {
    const y = await fetch(adres(k.port, yol), Object.assign({
      cache: 'no-store',
      headers: { 'X-Kasa': KIMLIK, 'Content-Type': 'application/json' },
    }, secenek || {}));
    return await y.json();
  } catch (e) {
    // Uygulama arada kapanmış olabilir; port önbelleği düşsün.
    PORT = null;
    return { ok: false, hata: 'kasa-kapali' };
  }
}

chrome.runtime.onMessage.addListener((mesaj, gonderen, cevapla) => {
  (async () => {
    try {
      if (mesaj.type === 'durum') {
        const k = await kopru();
        cevapla(k ? Object.assign({ ok: true, port: k.port }, k.durum)
                  : { ok: false, hata: 'kasa-kapali' });
      } else if (mesaj.type === 'eslesenler') {
        cevapla(await cagir('/eslesenler?url=' + encodeURIComponent(mesaj.url || '')));
      } else if (mesaj.type === 'doldur') {
        const r = await cagir('/doldur', {
          method: 'POST',
          body: JSON.stringify({ id: parseInt(mesaj.id, 10) || 0, url: mesaj.url || '' }),
        });
        if (r && r.ok) kasaKapat();
        cevapla(r);
      } else if (mesaj.type === 'noSave') {
        kasaKapat();
        cevapla({ ok: true });
      } else {
        cevapla({ ok: false, hata: 'bilinmeyen istek' });
      }
    } catch (e) {
      cevapla({ ok: false, hata: String((e && e.message) || e) });
    }
  })();
  return true; // yanıt asenkron
});

/* Tarayıcının kendi şifre kaydetme/otomatik doldurma teklifini KAPAT.
   Kasadan doldurulan bilgiler Chrome'un şifre yöneticisine düşmez.
   'privacy' izniyle çalışır; verilmemişse sessizce geçer. */
function kasaKapat() {
  try {
    const s = chrome.privacy && chrome.privacy.services;
    if (!s) return;
    ['passwordSavingEnabled', 'autofillCreditCardEnabled', 'autofillAddressEnabled']
      .forEach((ad) => {
        if (s[ad]) s[ad].set({ value: false }, () => { void chrome.runtime.lastError; });
      });
  } catch (e) {}
}

/* Rozet: kasa kapalıysa ya da oturum yoksa kullanıcı bunu eklenti simgesinden
   görsün — sayfada hiçbir şey çıkmamasının nedenini aramasın. */
async function rozet() {
  const k = await kopru();
  if (!k) {
    chrome.action.setBadgeText({ text: '!' });
    chrome.action.setBadgeBackgroundColor({ color: '#c02a45' });
    chrome.action.setTitle({ title: 'Bogahost Kasa — uygulama kapalı' });
  } else if (!k.durum.oturum) {
    chrome.action.setBadgeText({ text: '!' });
    chrome.action.setBadgeBackgroundColor({ color: '#a9660a' });
    chrome.action.setTitle({ title: 'Bogahost Kasa — uygulamada oturum açık değil' });
  } else {
    chrome.action.setBadgeText({ text: '' });
    chrome.action.setTitle({ title: 'Bogahost Kasa' });
  }
}

chrome.runtime.onInstalled.addListener(() => { kasaKapat(); zamanla(); });
if (chrome.runtime.onStartup) chrome.runtime.onStartup.addListener(zamanla);
function zamanla() {
  if (chrome.alarms) chrome.alarms.create('kasaDurum', { periodInMinutes: 5 });
  rozet();
}
if (chrome.alarms) {
  chrome.alarms.onAlarm.addListener((a) => { if (a.name === 'kasaDurum') rozet(); });
}
