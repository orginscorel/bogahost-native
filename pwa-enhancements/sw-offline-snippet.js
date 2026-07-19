/* ============================================================================
 * BOGAHOST PWA — Offline fallback eklentisi (mevcut service worker'a EKLENECEK)
 * ----------------------------------------------------------------------------
 * Bu bir TAM service worker DEĞİL. Mevcut push-sw.js / sw.js dosyalarının
 * push/notification mantığını BOZMADAN, en alta eklenecek küçük bir parçadır.
 *
 * Ne yapar: navigasyon (sayfa) isteği ağ hatası alırsa (çevrimdışı),
 * önbelleğe alınmış /offline.html gösterilir. API/asset istekleri normal akar.
 * Mevcut çalışan hiçbir fetch/push davranışını değiştirmez (yalnız navigation +
 * network hatası durumunda devreye girer).
 * ============================================================================ */

(function () {
  var OFFLINE_URL = '/offline.html';
  var CACHE = 'bhx-offline-v1';

  self.addEventListener('install', function (event) {
    event.waitUntil(
      caches.open(CACHE).then(function (c) { return c.add(OFFLINE_URL); }).catch(function () {})
    );
    // self.skipWaiting() mevcut SW zaten çağırıyorsa TEKRAR ekleme.
  });

  self.addEventListener('activate', function (event) {
    // Eski offline cache sürümlerini temizle (yalnız kendi cache'imiz — başkasına dokunma).
    event.waitUntil(
      caches.keys().then(function (keys) {
        return Promise.all(keys.filter(function (k) {
          return k.indexOf('bhx-offline-') === 0 && k !== CACHE;
        }).map(function (k) { return caches.delete(k); }));
      })
    );
  });

  self.addEventListener('fetch', function (event) {
    var req = event.request;
    // YALNIZCA sayfa navigasyonları + ağ hatası → offline sayfası.
    if (req.mode !== 'navigate') return;
    event.respondWith(
      fetch(req).catch(function () {
        return caches.open(CACHE).then(function (c) { return c.match(OFFLINE_URL); });
      })
    );
  });
})();
