import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'node:fs'

const packageJson = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as {
  version: string
}
const allowedHosts = (process.env.ALLOWED_HOSTS ?? process.env.VITE_ALLOWED_HOSTS)
  ?.split(',')
  .map((host) => host.trim())
  .filter(Boolean) ?? []

export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    __TRACKER_VERSION__: JSON.stringify(packageJson.version),
    __FRONTEND_VERSION__: JSON.stringify(packageJson.version),
  },
  server: {
    allowedHosts,
    proxy: {
      '/api': {
        target: process.env.VITE_API_PROXY_TARGET ?? 'http://127.0.0.1:3124',
        changeOrigin: true,
      },
      '/api-docs': {
        target: process.env.VITE_API_PROXY_TARGET ?? 'http://127.0.0.1:3124',
        changeOrigin: true,
      },
      '/swagger-ui': {
        target: process.env.VITE_API_PROXY_TARGET ?? 'http://127.0.0.1:3124',
        changeOrigin: true,
      },
    },
  },
  preview: {
    allowedHosts,
  },
  test: {
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
  },
})
