# Eklenti alan-tespit testleri

`form-kaliplari.html` gerçek sayfalarda karşılaşılan on üç form kalıbını tek
sayfada topluyor. `autofill.js` bu sayfaya yüklenip her alana odaklanılıyor;
menünün **açılması gerektiği yerde açılması, açılmaması gerektiği yerde
açılmaması** ölçülüyor.

Neden gerekli: alan tespiti sezgisel bir iştir ve her düzeltme başka bir
kalıbı bozabilir. "Kupon kodunu kullanıcı adı sanmasın" derken "iki adımlı
girişte hiç açılmasın" hâline gelmesi kolay.

## Kalıplar

| # | kalıp | beklenen |
|---|---|---|
| 1 | klasik giriş (kullanıcı + parola) | açılır |
| 2 | araya arama ve kupon kutusu girmiş giriş | yalnız e-posta ve parolada açılır |
| 3 | iki adımlı giriş — parola alanı YOK | açılır (yalnız kullanıcı adı dolar) |
| 4 | kayıt formu (yeni parola + tekrar) | hiçbirinde açılmaz |
| 5 | OTP kutusu | açılmaz |
| 6 | arama ve şehir kutuları | açılmaz |

## Çalıştırma

Sayfa `chrome.runtime` yerine sahte bir uç kullanıyor; eklenti kurulu
olmadan çalışıyor.

    cd eklenti/testler && python3 -m http.server 8899
    # tarayıcıda http://127.0.0.1:8899/form-kaliplari.html

Her alana sırayla odaklanıp menünün açılıp açılmadığına bakın. Otomatik
koşum için `autofill.js`'i sayfanın yanına kopyalayıp `mode: 'closed'` →
`mode: 'open'` değiştirmek gerekiyor (gölge DOM'a dışarıdan bakabilmek için).
