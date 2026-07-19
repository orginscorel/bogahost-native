import type { CapacitorConfig } from '@capacitor/cli';

/**
 * Bogahost Finans — Capacitor 7 kabuk yapılandırması.
 *
 * Mimari: Bu bir WebView kabuğudur. `server.url` CANLI PWA'yı yükler; uygulamanın
 * kendi HTML/JS'i (www/) yalnızca uzak sunucu yüklenene kadar gösterilen bir
 * bootstrap ekranıdır. Backend/frontend koduna DOKUNULMAZ.
 */
const config: CapacitorConfig = {
  appId: 'com.bogahost.finans',
  appName: 'Bogahost Finans',
  webDir: 'www',
  backgroundColor: '#0e1015',
  server: {
    // CANLI URL — WebView doğrudan buraya gider.
    url: 'https://finans.bogahost.com/admin',
    // 4 Bogahost sistemi arasında AYNI kabukta geçiş yapılabilsin diye
    // dördünün de host'u izinli (bkz. capacitor/README.md "Uygulamalar arası geçiş").
    // Dışarıya (3. taraf) gezinme hâlâ engelli — sistem tarayıcısında açılır.
    allowNavigation: [
      'finans.bogahost.com',
      'dcim.bogahost.com',
      'chat.bogahost.com',
      'task.bogahost.com',
    ],
    // Düz HTTP yok — sadece TLS.
    cleartext: false,
    androidScheme: 'https',
  },
  android: {
    backgroundColor: '#0e1015',
    // release AAB/APK CI'da imzalanır (bkz. capacitor/README.md).
    allowMixedContent: false,
  },
  ios: {
    backgroundColor: '#0e1015',
    contentInset: 'always',
  },
  plugins: {
    SplashScreen: {
      launchShowDuration: 800,
      launchAutoHide: true,
      backgroundColor: '#0e1015',
      showSpinner: false,
      androidSpinnerStyle: 'small',
      iosSpinnerStyle: 'small',
      splashFullScreen: true,
      splashImmersive: true,
    },
    StatusBar: {
      // Koyu arka plan → açık (beyaz) metin.
      style: 'DARK',
      backgroundColor: '#0e1015',
      overlaysWebView: false,
    },
    PushNotifications: {
      // NOT: Native push için FCM (Android) + APNs (.p8, iOS) kimlik bilgileri
      // MANUEL kurulur — bkz. capacitor/README.md "Native Push" bölümü.
      presentationOptions: ['badge', 'sound', 'alert'],
    },
  },
};

export default config;
