/* Bogahost Kasa — sayfa içi doldurma.

   ALANIN ÜSTÜNE OTURAN İKON KALDIRILDI.
   Önceki sürüm şifre kutusunun sağına mutlak konumlu bir anahtar resmi
   koyuyordu. Üç sorunu vardı: kutunun içeriğini örtüyordu, sayfa kaydıkça
   800 ms'de bir yeniden konumlanıyordu, ve tek yolu onun üstüne tıklamaktı.

   Yeni davranış: alana odaklandığınızda kutunun ALTINDA bir liste açılıyor,
   ok tuşları + Enter ile seçiliyor, Esc ile kapanıyor. Sayfanın üstünde
   hiçbir şey asılı kalmıyor. Panel gölge DOM içinde çiziliyor — sayfanın
   kendi CSS'i görünümünü bozamıyor.

   Eşleşme sorgusu TAM ADRESLE yapılıyor; uygulama alan adını da yolu da
   karşılaştırıyor ve YOL TAM UYMALI. Giriş yapıldıktan sonraki alt
   sayfalarda liste artık açılmıyor. */
(function () {
  if (window.top !== window) return;                    // yalnız üst çerçeve
  if (location.protocol !== 'https:' && location.protocol !== 'http:') return;

  /* SÜRÜM DAMGASI — eklenti kendini yeniledikten sonra açık sekmelere geri
     yerleşiyor. Aynı sürüm ikinci kez enjekte edilirse çıkılıyor; ESKİ bir
     sürüm çalışıyorsa önce o sökülüyor. Düz bir "yüklü mü" bayrağı yeni
     sürümün de girmesini engellerdi.
     İçerik betikleri aynı eklentinin yalıtılmış dünyasını paylaşıyor, o
     yüzden bu bayrak sayfaya sızmıyor. */
  var SURUM = (function () {
    try { return chrome.runtime.getManifest().version; } catch (e) { return '?'; }
  })();
  if (window.__bogahostKasa === SURUM) return;
  if (window.__bogahostKasa && typeof window.__bogahostKasaSok === 'function') {
    try { window.__bogahostKasaSok(); } catch (e) {}
  }
  window.__bogahostKasa = SURUM;

  /* Eklenti yeniden yüklenince bu betiğin köprüsü kopar; Chrome
     "Extension context invalidated" atar. Sessizce sökülüyoruz — yerimize
     yeni sürüm zaten enjekte ediliyor. */
  function gonder(mesaj, geri) {
    try {
      chrome.runtime.sendMessage(mesaj, function (c) {
        if (chrome.runtime.lastError) { sok(); return; }
        geri(c);
      });
    } catch (e) { sok(); }
  }

  var KAYITLAR = null;        // null = daha sorulmadı
  var sorguda = false;
  var kapatildi = false;      // kullanıcı bu sayfada kapattı
  var dolduruldu = false;     // bu sayfada dolduruldu, bir daha açma
  var sonAdres = location.href;
  var panel = null;           // { host, kok, alan, secili }

  // ── yardımcılar ────────────────────────────────────────────────────────
  function gorunur(el) {
    if (!el) return false;
    var r = el.getBoundingClientRect();
    if (r.width < 44 || r.height < 8) return false;
    var s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden' && s.opacity !== '0';
  }
  function sifreAlanlari(kok) {
    return Array.prototype.filter.call(
      (kok || document).querySelectorAll('input[type=password]'), gorunur);
  }

  /* KAPSAM: BİR ALAN HANGİ FORMA AİT?
     Bunu sayfa geneline bakarak yapmak yanlış sonuç veriyordu. Gerçek
     sayfalarda giriş formunun yanında bülten kaydı, arama kutusu ve
     "şifremi değiştir" formu bir arada durabiliyor. Sayfada dört parola
     alanı görünce "burası kayıt formu" demek, giriş formunu da kapatıyordu.
     Artık her alan KENDİ formunun içinde değerlendiriliyor. */
  function kapsam(el) {
    if (!el) return document;
    if (el.form) return el.form;
    var k = null;
    try { k = el.closest('form, fieldset, [role="form"], section, article'); } catch (e) {}
    return k || document;
  }
  /* ── ALAN TESPİT MOTORU ───────────────────────────────────────────────
     Önceki sürüm iki varsayıma dayanıyordu:
       1. Sayfada bir `input[type=password]` VARDIR.
       2. Kullanıcı adı, ondan ÖNCEKİ son metin kutusudur.

     İkisi de sık sık yanlış. İki adımlı girişlerde (önce e-posta, sonra
     şifre) ilk adımda parola alanı YOK — eklenti hiç açılmıyordu. "Sonraki
     metin kutusu" kuralı da arama kutusunu, kupon kodunu, ülke seçimini
     kullanıcı adı sanabiliyor.

     Artık alanlar PUANLANIYOR: autocomplete, type, name, id, placeholder,
     aria-label ve etiket metni birlikte değerlendiriliyor. `autocomplete`
     en güçlü işaret çünkü sayfa yazarının açık beyanı. */

  function nitelikler(el) {
    var p = [
      el.getAttribute('autocomplete'), el.getAttribute('name'), el.id,
      el.getAttribute('placeholder'), el.getAttribute('aria-label'),
      el.getAttribute('data-testid'),
    ];
    // Bağlı etiketin metni de bir işaret — `<label for=...>` ya da sarmalayan label.
    try {
      if (el.id) {
        var lb = document.querySelector('label[for="' + CSS.escape(el.id) + '"]');
        if (lb) p.push(lb.textContent);
      }
      var sarma = el.closest('label');
      if (sarma) p.push(sarma.textContent);
    } catch (e) {}
    return p.filter(Boolean).join(' ').toLowerCase();
  }

  var KULLANICI_ISARET = /(user|kullanic|kullanıc|login|giris|giriş|email|e-mail|eposta|e-posta|mail|account|hesap|uye|üye|tckn|phone|telefon|msisdn)/;
  var KULLANICI_KARSI = /(search|ara|arama|coupon|kupon|promo|zip|posta.?kod|city|sehir|şehir|address|adres|card|kart|cvv|amount|tutar|quantity|adet|comment|yorum|subject|konu|otp|code|kod)/;
  var YENI_PAROLA = /(new.?pass|yeni.?[sş]ifre|yeni.?parola|confirm|tekrar|repeat|again|retype|dogrula|doğrula|register|kayit|kayıt|signup|sign-up)/;

  /** Bir alanın kullanıcı-adı olma puanı. Negatif = değil. */
  function kullaniciPuani(el) {
    var t = (el.getAttribute('type') || 'text').toLowerCase();
    if (['password', 'hidden', 'checkbox', 'radio', 'submit', 'button', 'file', 'range', 'color'].indexOf(t) >= 0) return -100;
    if (!gorunur(el)) return -100;

    var n = nitelikler(el);
    var ac = (el.getAttribute('autocomplete') || '').toLowerCase();
    var puan = 0;

    // Sayfa yazarının açık beyanı en güçlü işaret.
    if (ac === 'username' || ac === 'email') puan += 60;
    else if (ac === 'off' || ac === 'new-password') puan -= 10;
    if (ac.indexOf('one-time-code') >= 0) return -100;   // OTP kutusu

    if (t === 'email') puan += 30;
    else if (t === 'tel') puan += 8;
    else if (t === 'text' || t === '') puan += 4;
    else return -100;                                    // number, date, url…

    if (KULLANICI_ISARET.test(n)) puan += 25;
    if (KULLANICI_KARSI.test(n)) puan -= 45;

    // Bir formun içinde olmak iyi işaret; sayfanın tepesindeki serbest
    // arama kutusu genelde formsuz durur.
    if (el.form) puan += 6;
    if (el.getAttribute('maxlength') === '1') return -100;  // OTP hücresi
    return puan;
  }

  /** Şifre alanı gerçekten GİRİŞ parolası mı, yoksa yeni parola mı? */
  function yeniParolaMi(pw) {
    var ac = (pw.getAttribute('autocomplete') || '').toLowerCase();
    if (ac === 'new-password') return true;
    if (ac === 'current-password') return false;
    if (YENI_PAROLA.test(nitelikler(pw))) return true;
    // AYNI FORMDA iki görünür parola alanı = kayıt ya da parola değiştirme.
    // Sayfa geneline bakmak, yandaki kayıt formu yüzünden giriş formunu da
    // kapatıyordu.
    return sifreAlanlari(kapsam(pw)).length >= 2;
  }

  /**
   * Şifre alanına ait kullanıcı adı kutusu.
   *
   * Şifreden ÖNCE gelen adaylar arasından en yüksek puanlı olan seçiliyor.
   * "En sondaki metin kutusu" kuralı, araya giren bir arama ya da kupon
   * kutusunu kullanıcı adı sanıyordu.
   */
  function kullaniciAlani(pw) {
    var liste = kapsam(pw).querySelectorAll('input');
    var eniyi = null, eniyiPuan = 0;
    for (var i = 0; i < liste.length; i++) {
      var c = liste[i];
      if (c === pw) break;
      var p = kullaniciPuani(c);
      if (p > eniyiPuan) { eniyiPuan = p; eniyi = c; }
    }
    return eniyi;
  }

  /**
   * İKİ ADIMLI GİRİŞ: sayfada parola alanı yok ama net bir kullanıcı adı
   * kutusu var. Google, Microsoft ve pek çok panel böyle çalışıyor;
   * önceki sürüm bu adımda hiç açılmıyordu.
   *
   * Eşik yüksek tutuldu (40): şüphede kalınca AÇMAMAK doğru davranış —
   * her metin kutusunda menü açan bir eklenti, kapatılan bir eklentidir.
   */
  function tekBasinaKullaniciAlani(el) {
    var k = kapsam(el);
    if (sifreAlanlari(k).length) return null;
    var liste = k.querySelectorAll('input');
    var eniyi = null, eniyiPuan = 39;
    for (var i = 0; i < liste.length; i++) {
      var p = kullaniciPuani(liste[i]);
      if (p > eniyiPuan) { eniyiPuan = p; eniyi = liste[i]; }
    }
    return eniyi;
  }

  function deger(el, v) {
    try {
      var proto = el.tagName === 'TEXTAREA'
        ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, v);
    } catch (e) { el.value = v; }
    ['input', 'change', 'keyup', 'blur'].forEach(function (ev) {
      el.dispatchEvent(new Event(ev, { bubbles: true }));
    });
  }
  /** Tarayıcının "şifreyi kaydedeyim mi" balonunu bastır. */
  function kaydetme(pw) {
    try {
      if (pw.form) pw.form.setAttribute('autocomplete', 'off');
      pw.setAttribute('autocomplete', 'new-password');
      var u = kullaniciAlani(pw);
      if (u) u.setAttribute('autocomplete', 'off');
    } catch (e) {}
    gonder({ type: 'noSave' }, function () {});
  }
  function hataMetni(r) {
    var h = r && (r.hata || r.error);
    if (h === 'kasa-kapali') return 'Bogahost Kasa uygulaması kapalı — açın.';
    if (h === 'oturum yok') return 'Kasa uygulamasında oturum açık değil.';
    return h || 'şifre alınamadı';
  }

  // ── bildirim ───────────────────────────────────────────────────────────
  var bildirimHost = null, bildirimZaman = null;
  function bildir(metin, iyi) {
    if (!bildirimHost) {
      bildirimHost = document.createElement('div');
      bildirimHost.style.cssText = 'all:initial;position:fixed;z-index:2147483647;'
        + 'left:50%;bottom:24px;transform:translateX(-50%)';
      var k = bildirimHost.attachShadow({ mode: 'closed' });
      k.innerHTML = '<style>' + TEMA + `
        .b{font:600 13px/1.4 ${YAZI};padding:10px 16px;border-radius:12px;
           background:var(--kart);color:var(--metin);border:1px solid var(--cizgi);
           box-shadow:var(--golge);opacity:0;transition:opacity .22s;max-width:80vw}
        .b.gor{opacity:1}
        .b.iyi{border-color:var(--iyi);color:var(--iyi)}
      </style><div class="b" id="b"></div>`;
      bildirimHost._b = k.getElementById('b');
      document.documentElement.appendChild(bildirimHost);
    }
    var b = bildirimHost._b;
    b.textContent = metin;
    b.className = 'b gor' + (iyi ? ' iyi' : '');
    clearTimeout(bildirimZaman);
    bildirimZaman = setTimeout(function () { b.className = 'b'; }, 2800);
  }

  // ── görünüm ────────────────────────────────────────────────────────────
  var YAZI = '-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,system-ui,sans-serif';
  /* Açık tema tam tanımlı; koyu tema yalnız değişkenleri yeniden yazıyor.
     Panel sayfanın değil, KULLANICININ temasını izliyor. */
  var TEMA = `
    *{box-sizing:border-box}
    :host{
      --kart:#ffffff; --kart2:#f4f3fa; --metin:#18172a; --silik:#8f8ca6;
      --cizgi:#e3e0ee; --vurgu:#5443D2; --iyi:#12905a;
      --golge:0 6px 16px rgba(20,16,45,.10), 0 18px 44px rgba(20,16,45,.14);
    }
    @media (prefers-color-scheme:dark){
      :host{
        --kart:#1b1b24; --kart2:#24242f; --metin:#efeef7; --silik:#8b8899;
        --cizgi:#31303f; --vurgu:#8F86F0; --iyi:#4ecb8d;
        --golge:0 6px 16px rgba(0,0,0,.5), 0 20px 50px rgba(0,0,0,.55);
      }
    }`;

  function panelKapat() {
    if (!panel) return;
    document.removeEventListener('mousedown', panel.disari, true);
    window.removeEventListener('scroll', panel.tasi, true);
    window.removeEventListener('resize', panel.tasi);
    panel.host.remove();
    panel = null;
  }

  function panelAc(alan, kayitlar) {
    panelKapat();

    var host = document.createElement('div');
    host.style.cssText = 'all:initial;position:fixed;z-index:2147483647';
    var kok = host.attachShadow({ mode: 'closed' });
    kok.innerHTML = '<style>' + TEMA + `
      .k{font:400 13px/1.45 ${YAZI};background:var(--kart);color:var(--metin);
         border:1px solid var(--cizgi);border-radius:13px;box-shadow:var(--golge);
         overflow:hidden;animation:ac .13s ease-out}
      @keyframes ac{from{opacity:0;transform:translateY(-4px)}to{opacity:1;transform:none}}
      @media (prefers-reduced-motion:reduce){.k{animation:none}}
      .bas{display:flex;align-items:center;gap:7px;padding:9px 11px 7px;
           font-size:10.5px;font-weight:700;letter-spacing:.5px;text-transform:uppercase;
           color:var(--silik)}
      .bas svg{width:13px;height:13px;stroke:var(--vurgu);fill:none;stroke-width:2.2;
               stroke-linecap:round;stroke-linejoin:round;flex:none}
      .bas .bosluk{flex:1}
      .kapat{all:unset;cursor:pointer;padding:2px 5px;border-radius:6px;color:var(--silik);
             font-size:14px;line-height:1}
      .kapat:hover{background:var(--kart2);color:var(--metin)}
      .satir{display:flex;align-items:center;gap:10px;width:100%;text-align:left;
             padding:10px 11px;border:0;background:none;color:inherit;font:inherit;cursor:pointer}
      .satir:hover,.satir.sec{background:var(--kart2)}
      .satir.sec{box-shadow:inset 3px 0 0 var(--vurgu)}
      .im{width:28px;height:28px;border-radius:9px;flex:none;display:flex;align-items:center;
          justify-content:center;color:#fff;font-weight:700;font-size:12px;background:var(--vurgu)}
      .m{display:block;min-width:0;flex:1}
      .ad{display:block;font-weight:600;line-height:1.35;
           overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
      .alt{display:block;font-size:11.5px;line-height:1.35;color:var(--silik);
           overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
      .ayak{padding:7px 11px 9px;font-size:10.5px;color:var(--silik);border-top:1px solid var(--cizgi)}
    </style>
    <div class="k" part="k">
      <div class="bas">
        <svg viewBox="0 0 24 24"><rect x="4" y="10.5" width="16" height="10.5" rx="2.5"/><path d="M8 10.5V7.5a4 4 0 0 1 8 0v3"/></svg>
        <span>Bogahost Kasa</span><span class="bosluk"></span>
        <button class="kapat" id="kapat" title="Kapat (Esc)">&times;</button>
      </div>
      <div id="liste"></div>
      <div class="ayak">Enter ile doldur · Esc ile kapat</div>
    </div>`;

    var liste = kok.getElementById('liste');
    kayitlar.forEach(function (k, i) {
      var d = document.createElement('button');
      d.type = 'button';
      d.className = 'satir' + (i === 0 ? ' sec' : '');
      var ad = k.etiket || 'Giriş';
      d.innerHTML = '<span class="im"></span><span class="m">'
        + '<span class="ad"></span><span class="alt"></span></span>';
      d.querySelector('.im').textContent = ad.charAt(0).toUpperCase();
      d.querySelector('.ad').textContent = ad;
      d.querySelector('.alt').textContent = k.kullanici || k.alan || '';
      d.addEventListener('mousedown', function (e) { e.preventDefault(); });
      d.addEventListener('click', function (e) {
        e.preventDefault(); e.stopPropagation(); doldur(alan, k);
      });
      liste.appendChild(d);
    });

    kok.getElementById('kapat').addEventListener('click', function (e) {
      e.preventDefault(); kapatildi = true; panelKapat();
    });

    document.documentElement.appendChild(host);

    panel = {
      host: host, kok: kok, alan: alan, kayitlar: kayitlar, secili: 0,
      tasi: function () { yerlestir(host, alan); },
      disari: function (e) { if (e.target !== alan && !host.contains(e.target)) panelKapat(); },
    };
    yerlestir(host, alan);
    document.addEventListener('mousedown', panel.disari, true);
    window.addEventListener('scroll', panel.tasi, true);
    window.addEventListener('resize', panel.tasi);
  }

  /** Kutunun ALTINA yerleştir; yer yoksa üstüne al. */
  function yerlestir(host, alan) {
    var r = alan.getBoundingClientRect();
    if (!r.width) { panelKapat(); return; }
    var g = Math.min(Math.max(r.width, 250), 360);
    var x = Math.min(Math.max(8, r.left), window.innerWidth - g - 8);
    host.style.width = g + 'px';
    host.style.left = x + 'px';

    var y = kok_yuksekligi(host);
    if (r.bottom + 6 + y > window.innerHeight && r.top - 6 - y > 0) {
      host.style.top = (r.top - 6 - y) + 'px';
    } else {
      host.style.top = (r.bottom + 6) + 'px';
    }
  }
  function kok_yuksekligi(host) {
    var h = host.getBoundingClientRect().height;
    return h || 160;
  }

  function secimTasi(yon) {
    if (!panel) return;
    var satirlar = panel.kok.getElementById('liste').children;
    if (!satirlar.length) return;
    satirlar[panel.secili].classList.remove('sec');
    panel.secili = (panel.secili + yon + satirlar.length) % satirlar.length;
    satirlar[panel.secili].classList.add('sec');
  }

  // ── doldurma ───────────────────────────────────────────────────────────
  function doldur(alan, kayit) {
    var pw = alan.type === 'password' ? alan : (sifreAlanlari(kapsam(alan))[0] || null);
    panelKapat();
    gonder({ type: 'doldur', id: kayit.id, url: location.href }, function (r) {
      if (!r || !r.ok) { bildir('Kasa: ' + hataMetni(r), false); return; }

      if (pw) {
        kaydetme(pw);
        var u = kullaniciAlani(pw);
        if (u && r.kullanici) deger(u, r.kullanici);
        if (r.parola) deger(pw, r.parola);
        dolduruldu = true;
        bildir('✓ ' + (kayit.etiket || 'Giriş') + ' dolduruldu', true);
        return;
      }

      /* İKİ ADIMLI GİRİŞİN BİRİNCİ ADIMI — parola alanı yok.
         YALNIZ kullanıcı adı yazılıyor. `dolduruldu` işaretlenmiyor:
         ikinci adımda parola kutusu geldiğinde menü yine açılmalı. */
      if (r.kullanici) {
        deger(alan, r.kullanici);
        bildir('✓ Kullanıcı adı dolduruldu — parola bir sonraki adımda', true);
      } else {
        bildir('Bu kayıtta kullanıcı adı yok', false);
      }
    });
  }

  // ── akış ───────────────────────────────────────────────────────────────
  function ilgiliAlan(el) {
    if (!el || el.tagName !== 'INPUT' || !gorunur(el)) return null;

    if (el.type === 'password') {
      /* YENİ PAROLA FORMUNDA DOLDURMA ÖNERMİYORUZ.
         Kayıt olurken ya da parola değiştirirken mevcut parolayı basmak
         yanlış: kullanıcı YENİ bir parola koyuyor. Orada doğru davranış
         üreteci sunmak — Faz 4. Şimdilik karışmıyoruz. */
      return yeniParolaMi(el) ? null : el;
    }

    // KENDİ FORMUNDAKİ parola alanı — sayfadaki ilki değil.
    var pw = sifreAlanlari(kapsam(el))[0];
    if (pw) {
      /* KAYIT FORMUNDA KULLANICI ADI DA SUNULMUYOR.
         Parola alanları "yeni parola" diye reddediliyordu ama aynı formun
         e-posta kutusu hâlâ menü açıyordu. Yarım bir kural: kullanıcı
         e-postasını doldurup sonra parola kutusunda menüyü bulamıyor.
         Kayıt formu kayıt formudur — orada mevcut bir hesabı sunmuyoruz. */
      if (yeniParolaMi(pw)) return null;
      return kullaniciAlani(pw) === el ? el : null;
    }

    // İki adımlı giriş: bu formda parola alanı yok, kullanıcı adı kutusu var.
    return tekBasinaKullaniciAlani(el) === el ? el : null;
  }

  function odaklandi(e) {
    if (kapatildi || dolduruldu) return;
    var alan = ilgiliAlan(e.target);
    if (!alan) return;
    if (KAYITLAR === null) { getir(alan); return; }
    if (KAYITLAR.length) panelAc(alan, KAYITLAR);
  }

  function getir(alan) {
    if (sorguda) return;
    sorguda = true;
    gonder({ type: 'eslesenler', url: location.href }, function (r) {
      sorguda = false;
      KAYITLAR = (r && r.ok) ? (r.kayitlar || []) : [];
      if (!KAYITLAR.length || kapatildi || dolduruldu) return;
      // Odak hâlâ aynı alandaysa aç; kullanıcı başka yere geçtiyse rahatsız etme.
      if (document.activeElement === alan) panelAc(alan, KAYITLAR);
    });
  }

  function yazmaya_basladi(e) {
    if (panel && e.target === panel.alan) panelKapat();
  }
  document.addEventListener('focusin', odaklandi, true);
  document.addEventListener('input', yazmaya_basladi, true);

  function tusa_basildi(e) {
    if (!panel) return;
    if (e.key === 'Escape') { kapatildi = true; panelKapat(); e.stopPropagation(); }
    else if (e.key === 'ArrowDown') { secimTasi(1); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { secimTasi(-1); e.preventDefault(); }
    else if (e.key === 'Enter') {
      var k = panel.kayitlar[panel.secili];
      if (k) { e.preventDefault(); doldur(panel.alan, k); }
    }
  }
  document.addEventListener('keydown', tusa_basildi, true);

  /* Eklenti penceresinden "şunu doldur" isteği. */
  function pencereden(m, gonderen, cevapla) {
    if (!m || m.type !== 'kasa-doldur') return;
    var pw = sifreAlanlari()[0];
    if (!pw) { bildir('Sayfada şifre alanı bulunamadı', false); cevapla({ ok: false }); return; }
    var kayit = null;
    if (KAYITLAR) kayit = KAYITLAR.filter(function (i) { return i.id === m.id; })[0];
    doldur(pw, kayit || { id: m.id, etiket: 'Giriş' });
    cevapla({ ok: true });
  }
  try { chrome.runtime.onMessage.addListener(pencereden); } catch (e) {}

  /* TEK SAYFA UYGULAMALARDA ADRES DEĞİŞİNCE HER ŞEY SIFIRLANIR.
     Giriş yaptıktan sonra adres değişiyor ama sayfa yeniden yüklenmiyor;
     eski eşleşmeleri taşımak, kaydı ait olmadığı sayfada sunmak demekti. */
  var adresSaati = setInterval(function () {
    if (location.href === sonAdres) return;
    sonAdres = location.href;
    KAYITLAR = null; kapatildi = false; dolduruldu = false;
    panelKapat();
  }, 700);

  /* Kendini tamamen söker: yerine yeni sürüm gelirken ya da eklenti
     yeniden yüklenirken sayfada iz bırakmamak için. */
  function sok() {
    try { clearInterval(adresSaati); } catch (e) {}
    document.removeEventListener('focusin', odaklandi, true);
    document.removeEventListener('input', yazmaya_basladi, true);
    document.removeEventListener('keydown', tusa_basildi, true);
    try { chrome.runtime.onMessage.removeListener(pencereden); } catch (e) {}
    panelKapat();
    if (bildirimHost) { bildirimHost.remove(); bildirimHost = null; }
    if (window.__bogahostKasa === SURUM) window.__bogahostKasa = null;
  }
  window.__bogahostKasaSok = sok;
})();
