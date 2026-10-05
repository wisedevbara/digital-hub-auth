import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  // Top-level: assets are emitted as /idp/... because nginx serves the SPA
  // under /idp, not at the site root.
  base: '/idp/',
  build: {
    outDir: 'dist',
    emptyOutDir: true
  },
  server: {
    host: true,
    port: 5173
  }
})