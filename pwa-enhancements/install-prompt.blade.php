{{--
  BOGAHOST PWA — "Uygulamayı Yükle" istemi (yeniden kullanılabilir partial).
  Kullanım: her uygulamanın admin layout'unda </body> ÖNCESİNE @include('partials.install-prompt')
  Bağımlılık YOK (vanilla JS). Zaten kurulu (standalone) ise hiç görünmez.
  - Android/Chrome/Edge: beforeinstallprompt yakalanır → "Yükle" butonu.
  - iOS Safari (beforeinstallprompt yok): Paylaş → Ana Ekrana Ekle yönergesi.
  Kapatınca 14 gün tekrar göstermez (localStorage).
--}}
<div id="bhxInstall" style="display:none;position:fixed;left:12px;right:12px;bottom:12px;z-index:9999;
     max-width:460px;margin:0 auto;background:#171a22;color:#e6e8ee;border:1px solid rgba(255,255,255,.08);
     border-radius:16px;padding:14px 16px;box-shadow:0 16px 48px rgba(0,0,0,.45);
     font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,'Inter',sans-serif;
     padding-bottom:calc(14px + env(safe-area-inset-bottom))">
  <div style="display:flex;align-items:center;gap:12px">
    <div style="width:42px;height:42px;border-radius:12px;flex:0 0 auto;overflow:hidden;background:#5443D2">
      <img src="/icon-192.png" alt="" style="width:100%;height:100%;object-fit:cover">
    </div>
    <div style="flex:1;min-width:0">
      <div style="font-weight:700;font-size:14px">Uygulamayı yükle</div>
      <div id="bhxInstallHint" style="font-size:12px;color:#9aa1b2;line-height:1.4">Ana ekrana ekleyip tam ekran, hızlı erişin.</div>
    </div>
    <button id="bhxInstallBtn" type="button" style="appearance:none;border:0;cursor:pointer;background:#5443D2;
      color:#fff;font-weight:600;font-size:13px;padding:9px 16px;border-radius:10px;flex:0 0 auto">Yükle</button>
    <button id="bhxInstallX" type="button" aria-label="Kapat" style="appearance:none;border:0;cursor:pointer;
      background:transparent;color:#6b7280;font-size:20px;line-height:1;padding:4px 6px;flex:0 0 auto">&times;</button>
  </div>
</div>
<script>
(function () {
  var KEY = 'bhx_install_dismissed', box = document.getElementById('bhxInstall');
  if (!box) return;
  var standalone = window.matchMedia('(display-mode: standalone)').matches || window.navigator.standalone === true;
  var dismissed = (function () { try { var t = +localStorage.getItem(KEY) || 0; return Date.now() - t < 14 * 864e5; } catch (e) { return false; } })();
  if (standalone || dismissed) return;

  var btn = document.getElementById('bhxInstallBtn'), x = document.getElementById('bhxInstallX'),
      hint = document.getElementById('bhxInstallHint'), deferred = null;
  var isIOS = /iphone|ipad|ipod/i.test(navigator.userAgent) && !window.MSStream;

  function show() { box.style.display = ''; }
  function hide(remember) { box.style.display = 'none'; if (remember) { try { localStorage.setItem(KEY, Date.now()); } catch (e) {} } }

  window.addEventListener('beforeinstallprompt', function (e) {
    e.preventDefault(); deferred = e; btn.style.display = ''; show();
  });
  window.addEventListener('appinstalled', function () { hide(true); });

  if (isIOS) {
    // iOS Safari beforeinstallprompt tetiklemez → manuel yönerge göster.
    btn.style.display = 'none';
    hint.innerHTML = 'Paylaş <b>&#x2191;</b> &rarr; <b>Ana Ekrana Ekle</b> ile yükleyin.';
    setTimeout(show, 1200);
  }

  btn.addEventListener('click', function () {
    if (!deferred) return;
    deferred.prompt();
    deferred.userChoice.finally(function () { deferred = null; hide(false); });
  });
  x.addEventListener('click', function () { hide(true); });
})();
</script>
