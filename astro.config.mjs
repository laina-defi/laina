import { defineConfig } from 'astro/config';
import basicSsl from '@vitejs/plugin-basic-ssl';
import react from '@astrojs/react';

import tailwind from '@astrojs/tailwind';

// https://astro.build/config
export default defineConfig({
  site: 'https://www.laina-de.fi',
  vite: {
    plugins: [basicSsl()],
    server: {
      https: true,
    },
    define: {
      global: 'globalThis',
    },
  },
  output: 'static',
  integrations: [react(), tailwind()],
});
