// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  srcDir: 'app',
  compatibilityDate: '2024-11-01',

  future: {
    compatibilityVersion: 4,
  },

  ssr: true,

  modules: ['@nuxtjs/seo', '@nuxtjs/i18n', '@nuxt/content'],

  site: {
    url: 'https://whis.ink',
    name: 'whis',
    description: 'Voice-to-text for Linux',
    defaultLocale: 'en',
  },

  sitemap: {},

  robots: {
    allow: '/',
    sitemap: 'https://whis.ink/sitemap.xml',
  },

  i18n: {
    locales: [
      { code: 'en', iso: 'en-US', name: 'English', file: 'en.json', dir: 'ltr' },
      { code: 'zh', iso: 'zh-CN', name: '中文', file: 'zh.json', dir: 'ltr' },
      { code: 'es', iso: 'es-ES', name: 'Español', file: 'es.json', dir: 'ltr' },
      { code: 'fr', iso: 'fr-FR', name: 'Français', file: 'fr.json', dir: 'ltr' },
      { code: 'de', iso: 'de-DE', name: 'Deutsch', file: 'de.json', dir: 'ltr' },
      { code: 'pt', iso: 'pt-PT', name: 'Português', file: 'pt.json', dir: 'ltr' },
      { code: 'ru', iso: 'ru-RU', name: 'Русский', file: 'ru.json', dir: 'ltr' },
      { code: 'ja', iso: 'ja-JP', name: '日本語', file: 'ja.json', dir: 'ltr' },
      { code: 'ko', iso: 'ko-KR', name: '한국어', file: 'ko.json', dir: 'ltr' },
      { code: 'it', iso: 'it-IT', name: 'Italiano', file: 'it.json', dir: 'ltr' },
    ],
    defaultLocale: 'en',
    strategy: 'prefix_except_default',
    langDir: '../i18n/locales',
    vueI18n: './i18n.config.ts',
    compilation: {
      strictMessage: false,
      escapeHtml: false,
    },
    detectBrowserLanguage: {
      useCookie: true,
      cookieKey: 'i18n_redirected',
      redirectOn: 'root',
      alwaysRedirect: false,
      fallbackLocale: 'en',
    },
  },

  css: [
    '~/assets/tokens.css',
    '~/assets/base.css',
  ],

  app: {
    head: {
      charset: 'utf-8',
      viewport: 'width=device-width, initial-scale=1',
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
      ],
    },
    baseURL: '/',
  },

  runtimeConfig: {
    public: {
      appVersion: '0.7.2',
    },
  },

  nitro: {
    static: true,
    output: {
      publicDir: 'dist',
    },
    prerender: {
      crawlLinks: true, // Auto-discover all locale routes
      routes: [
        // English routes (default, no prefix)
        '/',
        '/downloads',
        '/cli',
        '/desktop',
        '/mobile',
        '/faq',
        '/blog',
        // Localized routes for all other languages
        ...['zh', 'es', 'fr', 'de', 'pt', 'ru', 'ja', 'ko', 'it'].flatMap(locale =>
          ['/', '/downloads', '/cli', '/desktop', '/mobile', '/faq', '/blog'].map(path =>
            `/${locale}${path}`,
          ),
        ),
      ],
      failOnError: false,
    },
  },

  experimental: {
    payloadExtraction: false,
    appManifest: false, // Explicitly disable to prevent #app-manifest import errors
    typedPages: true,
  },

  typescript: {
    strict: true,
    typeCheck: false, // Disable vite-plugin-checker (removed vue-tsc)
  },

  devtools: { enabled: false },

})
