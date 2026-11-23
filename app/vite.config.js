import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Ne pas surveiller la partie Rust : sous Windows, les .exe en cours de compilation
    // dans target/ sont verrouillés et font planter Vite (EBUSY).
    watch: { ignored: ['**/target/**', '**/src-tauri/**'] },
  },
});
