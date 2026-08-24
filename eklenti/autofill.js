/* Bogahost Kasa — otomatik doldurma (tüm sitelerde çalışır).
   Sayfadaki giriş alanlarını bulur, BU BİLGİSAYARDAKİ Kasa uygulamasına bu
   adrese uyan kayıtları sorar, şifre alanının içine küçük bir ANAHTAR ikonu
   koyar. Tıklayınca kullanıcı adı + şifreyi doldurur. Eklenti popup'ına basmaya
   gerek yok. Ayrıca tarayıcının bu şifreyi kaydetmesini engeller.

   Eşleşme sorgusu TAM ADRESLE yapılıyor: uygulama hem alan adını hem YOLU
   karşılaştırıyor. Böylece "ornek.com/giris" kaydı sitenin başka
   sayfalarındaki alanlara sunulmuyor. */
(function () {
  if (window.top !== window) return;              // yalnız üst çerçeve
  if (location.protocol !== 'https:' && location.protocol !== 'http:') return;

  var CACHE = null;          // {items:[...]} bu sayfa için eşleşmeler
  var fetching = false;
  var attached = new WeakSet();

  // ---- yardımcılar ----
  function visible(el) {
    if (!el) return false;
    var r = el.getBoundingClientRect();
    if (r.width < 44 || r.height < 8) return false;
    var s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden' && s.opacity !== '0';
  }
  function passwords() {
    return Array.prototype.filter.call(
      document.querySelectorAll('input[type=password]'), visible
    );
  }
  function findUser(pw) {
    var scope = pw.form || document;
    var list = scope.querySelectorAll('input, select');
    var user = null;
    for (var i = 0; i < list.length; i++) {
      var c = list[i];
      if (c === pw) break;                        // şifreden ÖNCEKİ son metin alanı
      var t = (c.getAttribute('type') || 'text').toLowerCase();
      if ((t === 'text' || t === 'email' || t === 'tel' || t === '') && visible(c)) user = c;
    }
    return user;
  }
  function setVal(el, val) {
    try {
      var proto = el.tagName === 'TEXTAREA'
        ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
      var setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
      setter.call(el, val);
    } catch (e) { el.value = val; }
    ['input', 'change', 'keyup', 'blur'].forEach(function (ev) {
      el.dispatchEvent(new Event(ev, { bubbles: true }));
    });
  }
  function killSave(pw) {
    // Sezgisel: form + alanlara autocomplete=off. Asıl kesin çözüm: privacy API (arka planda).
    try {
      var f = pw.form; if (f) f.setAttribute('autocomplete', 'off');
      pw.setAttribute('autocomplete', 'new-password');
      var u = findUser(pw); if (u) u.setAttribute('autocomplete', 'off');
    } catch (e) {}
    try { chrome.runtime.sendMessage({ type: 'noSave' }); } catch (e) {}
  }

  var toastEl = null, toastT = null;
  function toast(msg, ok) {
    if (!toastEl) {
      toastEl = document.createElement('div');
      toastEl.style.cssText = 'position:fixed;z-index:2147483647;left:50%;bottom:22px;transform:translateX(-50%);'
        + 'background:#191c26;color:#eef0f7;border:1px solid #2a2e3d;border-radius:12px;padding:10px 16px;'
        + 'font:600 13px system-ui,-apple-system,Segoe UI,Roboto,sans-serif;box-shadow:0 12px 34px rgba(0,0,0,.5);'
        + 'max-width:80vw;opacity:0;transition:opacity .25s';
      document.documentElement.appendChild(toastEl);
    }
    toastEl.textContent = msg;
    toastEl.style.borderColor = ok ? '#2f9e6f' : '#2a2e3d';
    toastEl.style.opacity = '1';
    clearTimeout(toastT);
    toastT = setTimeout(function () { toastEl.style.opacity = '0'; }, 2600);
  }

  // ---- doldurma ----
  function fill(pw, item) {
    chrome.runtime.sendMessage({ type: 'doldur', id: item.id, url: location.href }, function (r) {
      if (!r || !r.ok) { toast('Kasa: ' + hataMetni(r), false); return; }
      killSave(pw);
      var u = findUser(pw);
      if (u && r.kullanici) setVal(u, r.kullanici);
      if (r.parola) setVal(pw, r.parola);
      toast('✓ ' + (item.etiket || 'Giriş') + ' dolduruldu', true);
    });
  }

  /* Kullanıcı "neden olmadı" diye aramasın: kasa kapalıysa ya da oturum
     yoksa bunu açıkça söylüyoruz. */
  function hataMetni(r) {
    var h = r && (r.hata || r.error);
    if (h === 'kasa-kapali') return 'Bogahost Kasa uygulaması kapalı — açın.';
    if (h === 'oturum yok') return 'Kasa uygulamasında oturum açık değil.';
    return h || 'şifre alınamadı';
  }

  // ---- alan içi anahtar ikonu + seçim menüsü ----
  var KEY_SVG = 'data:image/svg+xml;utf8,'
    + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#7c6cf6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="15" r="4"/><path d="M10.85 12.15 19 4M18 5l2 2M15 8l2 2"/></svg>');

  function menu(anchor, items, pw) {
    closeMenu();
    var m = document.createElement('div');
    m.id = 'bhx-vault-menu';
    var r = anchor.getBoundingClientRect();
    m.style.cssText = 'position:fixed;z-index:2147483647;min-width:220px;max-width:320px;'
      + 'background:#191c26;border:1px solid #2a2e3d;border-radius:12px;padding:6px;'
      + 'box-shadow:0 16px 40px rgba(0,0,0,.55);font:500 13px system-ui,-apple-system,Segoe UI,Roboto,sans-serif;'
      + 'top:' + Math.min(r.bottom + 6, window.innerHeight - 20) + 'px;left:' + Math.max(8, r.right - 260) + 'px;';
    var head = document.createElement('div');
    head.textContent = 'Bogahost Kasa — doldur';
    head.style.cssText = 'color:#9aa0bb;font-size:11px;padding:6px 8px 8px;letter-spacing:.3px';
    m.appendChild(head);
    items.forEach(function (it) {
      var row = document.createElement('button');
      row.type = 'button';
      row.style.cssText = 'display:flex;align-items:center;gap:10px;width:100%;text-align:left;'
        + 'background:none;border:none;color:#eef0f7;padding:9px 8px;border-radius:9px;cursor:pointer';
      row.onmouseenter = function () { row.style.background = '#242838'; };
      row.onmouseleave = function () { row.style.background = 'none'; };
      row.innerHTML = '<span style="width:26px;height:26px;border-radius:8px;background:#5443D2;display:flex;'
        + 'align-items:center;justify-content:center;color:#fff;font-weight:700;flex-shrink:0">'
        + esc((it.etiket || '?').charAt(0).toUpperCase()) + '</span>'
        + '<span style="min-width:0"><b style="display:block;font-size:13px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">'
        + esc(it.etiket || 'Giriş') + '</b><span style="color:#9aa0bb;font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;display:block">'
        + esc(it.kullanici || it.alan || '') + '</span></span>';
      row.onclick = function (e) { e.preventDefault(); e.stopPropagation(); closeMenu(); fill(pw, it); };
      m.appendChild(row);
    });
    document.documentElement.appendChild(m);
    setTimeout(function () { document.addEventListener('mousedown', onDoc, true); }, 0);
    function onDoc(e) { if (!m.contains(e.target)) closeMenu(); }
    m._onDoc = onDoc;
  }
  function closeMenu() {
    var m = document.getElementById('bhx-vault-menu');
    if (m) { if (m._onDoc) document.removeEventListener('mousedown', m._onDoc, true); m.remove(); }
  }
  function esc(s) { return String(s == null ? '' : s).replace(/[<>&"]/g, function (c) { return { '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' }[c]; }); }

  function badge(pw, items) {
    if (attached.has(pw)) return;
    attached.add(pw);
    // Alanın sağına konumlanan tıklanır anahtar ikonu
    var b = document.createElement('img');
    b.src = KEY_SVG;
    b.title = 'Bogahost Kasa ile doldur';
    b.style.cssText = 'position:absolute;width:22px;height:22px;cursor:pointer;z-index:2147483646;'
      + 'opacity:.9;filter:drop-shadow(0 1px 2px rgba(0,0,0,.4))';
    function place() {
      var r = pw.getBoundingClientRect();
      if (!r.width) { b.style.display = 'none'; return; }
      b.style.display = '';
      b.style.top = (window.scrollY + r.top + (r.height - 22) / 2) + 'px';
      b.style.left = (window.scrollX + r.right - 30) + 'px';
    }
    b.onclick = function (e) {
      e.preventDefault(); e.stopPropagation();
      if (items.length === 1) fill(pw, items[0]); else menu(b, items, pw);
    };
    document.documentElement.appendChild(b);
    place();
    var reflow = function () { place(); };
    window.addEventListener('scroll', reflow, true);
    window.addEventListener('resize', reflow);
    // alan görünürlüğü değişebilir → periyodik hafif konumlama
    var iv = setInterval(function () { if (!document.contains(pw)) { clearInterval(iv); b.remove(); } else place(); }, 800);
  }

  // ---- akış ----
  function scan() {
    var pw = passwords();
    if (!pw.length) return;
    if (!CACHE) { fetchMatches(); return; }
    if (!CACHE.items || !CACHE.items.length) return;
    var items = CACHE.items;
    pw.forEach(function (p) { badge(p, items); });

    // Tek aday varsa kullanıcı adını ön-dolduruyoruz — ŞİFREYİ DEĞİL.
    // Şifre yalnızca kullanıcı anahtar ikonuna bastığında geliyor: hangi
    // alana yazılacağı görülmeden şifre istemenin anlamı yok.
    if (items.length === 1 && items[0].kullanici) {
      pw.forEach(function (p) {
        var u = findUser(p);
        if (u && !u.value) setVal(u, items[0].kullanici);
      });
    }
  }
  function fetchMatches() {
    if (fetching) return; fetching = true;
    chrome.runtime.sendMessage({ type: 'eslesenler', url: location.href }, function (r) {
      fetching = false;
      CACHE = (r && r.ok) ? { items: r.kayitlar || [] } : { items: [] };
      if (CACHE.items.length) scan();
    });
  }

  /* Eklenti penceresinden "şunu doldur" isteği. Doldurmayı yine BURASI
     yapıyor: alanları gören taraf sayfadaki betik. */
  chrome.runtime.onMessage.addListener(function (m, gonderen, cevapla) {
    if (!m || m.type !== 'kasa-doldur') return;
    var pw = passwords()[0];
    if (!pw) { toast('Sayfada şifre alanı bulunamadı', false); cevapla({ ok: false }); return; }
    var kayit = null;
    if (CACHE && CACHE.items) {
      kayit = CACHE.items.filter(function (i) { return i.id === m.id; })[0];
    }
    fill(pw, kayit || { id: m.id, etiket: 'Giriş' });
    cevapla({ ok: true });
  });

  // İlk tarama + DOM değişimlerinde (SPA/geç yüklenen formlar) yeniden tara
  scan();
  var deb = null;
  var mo = new MutationObserver(function () { clearTimeout(deb); deb = setTimeout(scan, 400); });
  try { mo.observe(document.documentElement, { childList: true, subtree: true }); } catch (e) {}
  window.addEventListener('load', scan);
})();
