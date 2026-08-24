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
  function sifreAlanlari() {
    return Array.prototype.filter.call(
      document.querySelectorAll('input[type=password]'), gorunur);
  }
  /** Şifre alanından ÖNCEKİ son metin kutusu — kullanıcı adı odur. */
  function kullaniciAlani(pw) {
    var kapsam = pw.form || document;
    var liste = kapsam.querySelectorAll('input, select');
    var bulunan = null;
    for (var i = 0; i < liste.length; i++) {
      var c = liste[i];
      if (c === pw) break;
      var t = (c.getAttribute('type') || 'text').toLowerCase();
      if ((t === 'text' || t === 'email' || t === 'tel' || t === '') && gorunur(c)) bulunan = c;
    }
    return bulunan;
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
    try { chrome.runtime.sendMessage({ type: 'noSave' }); } catch (e) {}
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
    var pw = alan.type === 'password' ? alan : (sifreAlanlari()[0] || null);
    if (!pw) { bildir('Sayfada şifre alanı bulunamadı', false); return; }
    panelKapat();
    chrome.runtime.sendMessage({ type: 'doldur', id: kayit.id, url: location.href }, function (r) {
      if (!r || !r.ok) { bildir('Kasa: ' + hataMetni(r), false); return; }
      kaydetme(pw);
      var u = kullaniciAlani(pw);
      if (u && r.kullanici) deger(u, r.kullanici);
      if (r.parola) deger(pw, r.parola);
      dolduruldu = true;           // bu sayfada işimiz bitti
      bildir('✓ ' + (kayit.etiket || 'Giriş') + ' dolduruldu', true);
    });
  }

  // ── akış ───────────────────────────────────────────────────────────────
  function ilgiliAlan(el) {
    if (!el || el.tagName !== 'INPUT' || !gorunur(el)) return null;
    if (el.type === 'password') return el;
    var pw = sifreAlanlari()[0];
    return (pw && kullaniciAlani(pw) === el) ? el : null;
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
    chrome.runtime.sendMessage({ type: 'eslesenler', url: location.href }, function (r) {
      sorguda = false;
      KAYITLAR = (r && r.ok) ? (r.kayitlar || []) : [];
      if (!KAYITLAR.length || kapatildi || dolduruldu) return;
      // Odak hâlâ aynı alandaysa aç; kullanıcı başka yere geçtiyse rahatsız etme.
      if (document.activeElement === alan) panelAc(alan, KAYITLAR);
    });
  }

  document.addEventListener('focusin', odaklandi, true);

  document.addEventListener('input', function (e) {
    if (panel && e.target === panel.alan) panelKapat();
  }, true);

  document.addEventListener('keydown', function (e) {
    if (!panel) return;
    if (e.key === 'Escape') { kapatildi = true; panelKapat(); e.stopPropagation(); }
    else if (e.key === 'ArrowDown') { secimTasi(1); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { secimTasi(-1); e.preventDefault(); }
    else if (e.key === 'Enter') {
      var k = panel.kayitlar[panel.secili];
      if (k) { e.preventDefault(); doldur(panel.alan, k); }
    }
  }, true);

  /* Eklenti penceresinden "şunu doldur" isteği. */
  chrome.runtime.onMessage.addListener(function (m, gonderen, cevapla) {
    if (!m || m.type !== 'kasa-doldur') return;
    var pw = sifreAlanlari()[0];
    if (!pw) { bildir('Sayfada şifre alanı bulunamadı', false); cevapla({ ok: false }); return; }
    var kayit = null;
    if (KAYITLAR) kayit = KAYITLAR.filter(function (i) { return i.id === m.id; })[0];
    doldur(pw, kayit || { id: m.id, etiket: 'Giriş' });
    cevapla({ ok: true });
  });

  /* TEK SAYFA UYGULAMALARDA ADRES DEĞİŞİNCE HER ŞEY SIFIRLANIR.
     Giriş yaptıktan sonra adres değişiyor ama sayfa yeniden yüklenmiyor;
     eski eşleşmeleri taşımak, kaydı ait olmadığı sayfada sunmak demekti. */
  setInterval(function () {
    if (location.href === sonAdres) return;
    sonAdres = location.href;
    KAYITLAR = null; kapatildi = false; dolduruldu = false;
    panelKapat();
  }, 700);
})();
