import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Standard Tauri + Vite wiring: fixed dev port matching tauri.conf.json's
// `devUrl`, and file-watch tuned to ignore the Rust side of the project.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    outDir: 'dist',
  },
});
