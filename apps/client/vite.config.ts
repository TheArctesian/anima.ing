import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

// Vitest resolves svelte's server build by default, where runes do nothing in DOM tests; SvelteKit builds pick their own.
export default defineConfig(({ mode }) => ({
  plugins: [sveltekit()],
  resolve: mode === 'test' ? { conditions: ['browser'] } : {},
  test: { include: ['src/**/*.test.ts'] },
}));
