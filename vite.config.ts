import path from 'path'
import { defineConfig } from 'vitest/config'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import wasm from 'vite-plugin-wasm'

export default defineConfig({
  plugins: [tailwindcss(), react(), wasm()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  build: {
    chunkSizeWarningLimit: 1500,
    target: 'es2022',
  },

  server: {
    watch: {
      ignored: ['**/src-rust/target/**'],
    },
  },

  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    globals: true,
    exclude: ['node_modules', 'dist', 'src-rust', 'src/wasm-pkg'],
  },
})
