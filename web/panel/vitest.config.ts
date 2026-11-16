import { defineConfig, mergeConfig } from 'vitest/config';

import viteConfig from './vite.config.ts';

export default mergeConfig(
    viteConfig,
    defineConfig({
        test: {
            environment: 'jsdom',
            restoreMocks: true,
            alias: {
                'react-dom/test-utils': 'preact/test-utils',
                'use-sync-external-store/shim': 'preact/compat',
            },
        },
    }),
);
