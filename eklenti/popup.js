/* Bogahost Kasa — eklenti penceresi.

   BURADA GİRİŞ YOK. Kimlik bu bilgisayardaki Kasa uygulamasında; eklenti ona
   bağlanıyor. Bu pencerenin tek işi: bağlantı durumunu göstermek ve açık
   sekmeye uyan kayıtları listelemek. Şifre burada hiç görünmez — satıra
   basınca sayfadaki alanlara yazılır. */

const $ = (id) => document.getElementById(id);
const esc = (s) => { const d = document.createElement('div'); d.textContent = s == null ? '' : String(s); return d.innerHTML; };

function sor(mesaj) {
  return new Promise((coz) => {
    chrome.runtime.sendMessage(mesaj, (c) => { void chrome.runtime.lastError; coz(c); });
  });
}

function bilgi(baslik, metin) {
  $('govde').innerHTML = '<div class="bilgi"><b>' + esc(baslik) + '</b>' + esc(metin) + '</div>';
}

(async function () {
  const d = await sor({ type: 'durum' });

  if (!d || !d.ok) {
    $('baglanti').textContent = 'uygulama kapalı';
    $('isik').className = 'isik kapali';
    bilgi('Bogahost Kasa çalışmıyor',
      'Eklenti kimliğini bu bilgisayardaki Kasa uygulamasından alıyor. Uygulamayı açtığınızda bağlantı kendiliğinden kurulur.');
    return;
  }

  /* KENDİ SÜRÜMÜMÜZ DE YAZIYOR.
     "İndirdim ama 1.2.2 diyor" karışıklığı buradan doğdu: Chrome'da birden
     fazla kayıt olabiliyor ve hangisinin yüklü olduğu görünmüyordu. */
  var benim = chrome.runtime.getManifest().version;
  $('baglanti').textContent = 'eklenti ' + benim + ' · uygulama ' + (d.surum || '—');
  $('ayak').textContent = 'Kimlik bu bilgisayardaki Kasa uygulamasından geliyor.';

  if (!d.oturum) {
    $('isik').className = 'isik uyari';
    bilgi('Oturum açık değil', 'Kasa uygulamasında giriş yapın; eklenti aynı oturumu kullanır.');
    return;
  }
  $('isik').className = 'isik acik';

  const sekmeler = await chrome.tabs.query({ active: true, currentWindow: true });
  const sekme = sekmeler[0];
  const url = (sekme && sekme.url) || '';
  if (!/^https?:/.test(url)) {
    bilgi('Bu sayfada doldurma yok', 'Bir web sayfasında açın.');
    return;
  }

  const e = await sor({ type: 'eslesenler', url: url });
  if (!e || !e.ok) {
    bilgi('Eşleşme alınamadı', (e && e.hata) || 'Kasa uygulamasıyla konuşulamadı.');
    return;
  }
  const kayitlar = e.kayitlar || [];
  if (!kayitlar.length) {
    let alan = url;
    try { alan = new URL(url).hostname; } catch (x) {}
    bilgi('Bu adres için kayıt yok', alan);
    return;
  }

  $('govde').innerHTML = kayitlar.map((k) =>
    '<button class="satir" data-id="' + k.id + '">'
    + '<span class="rozet">' + esc((k.etiket || '?').charAt(0).toUpperCase()) + '</span>'
    + '<span class="sMetin"><span class="sAd">' + esc(k.etiket || 'Giriş') + '</span>'
    + '<span class="sAlt">' + esc(k.kullanici || k.alan || '') + '</span></span>'
    + '</button>').join('');

  document.querySelectorAll('[data-id]').forEach((b) => {
    b.onclick = async () => {
      b.disabled = true;
      // Doldurmayı SAYFADAKİ betik yapıyor: alanları o görüyor. Buradan
      // yalnızca "şu kaydı doldur" diye sesleniyoruz; şifre bu pencereye
      // hiç uğramıyor.
      chrome.tabs.sendMessage(sekme.id, { type: 'kasa-doldur', id: Number(b.dataset.id) }, () => {
        void chrome.runtime.lastError;
        window.close();
      });
    };
  });
})();
