import { defineConfig } from 'vite'

// The APK serves the web bundle through WebViewAssetLoader at
// https://appassets.androidplatform.net/assets/www/ (see MainActivity), so the
// build uses that absolute base and lands inside the Android assets folder.
export default defineConfig(({ command }) => ({
  base: command === 'build' ? '/assets/www/' : '/',
  // @turbodocx/html-to-docx's browser build still reads Node's `global`.
  define: {
    global: 'globalThis'
  },
  build: {
    outDir: 'android/app/src/main/assets/www',
    emptyOutDir: true,
    target: 'chrome110',
    chunkSizeWarningLimit: 8000,
    assetsInlineLimit: 0
  },
  server: {
    host: true,
    port: 5174
  }
}))
