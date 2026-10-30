import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// Em desenvolvimento, `npm run dev` repassa /api para o backend Rust (porta 8080).
export default defineConfig({
  plugins: [svelte()],
  server: { proxy: { '/api': 'http://127.0.0.1:8080', '/metrics': 'http://127.0.0.1:8080' } },
})
