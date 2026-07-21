import type { CapacitorConfig } from '@capacitor/cli';

/**
 * Bogahost Muhasebe — Capacitor 7 kabuk yapılandırması.
 *
 * Mimari: Bu bir WebView kabuğudur. `server.url` CANLI PWA'yı yükler; uygulamanın
 * kendi HTML/JS'i (www/) yalnızca uzak sunucu yüklenene kadar gösterilen bir
 * bootstrap ekranıdır. Backend/frontend koduna DOKUNULMAZ.
 */
const config: CapacitorConfig = {
  appId: 'com.bogahost.muh',
  appName: 'Bogahost Muhasebe',
  webDir: 'www',
  backgroundColor: '#0e1015',
  server: {
    url: 'https://muh.bogahost.com/admin',
    allowNavigation: [
      'finans.bogahost.com',
      'dcim.bogahost.com',
      'chat.bogahost.com',
      'task.bogahost.com',
      'muh.bogahost.com',
    ],
    cleartext: false,
    androidScheme: 'https',
  },
  android: {
    backgroundColor: '#0e1015',
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
