import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

// Live earthquake demo — proxies to earthquake-server (.NET) on :3003
export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      input: resolve(__dirname, 'index.earthquake.html'),
    },
  },
  server: {
    port: 5175,
    proxy: {
      '/api': {
        target:       'http://localhost:3003',
        changeOrigin: true,
      },
    },
  },
});
