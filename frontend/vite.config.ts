import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  appType: 'spa',
  server: {
    proxy: {
      '/api': 'http://localhost:3000'
    }
  }
})
