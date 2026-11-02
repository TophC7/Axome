import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

const target = process.env.AXOME_TARGET ?? 'desktop'
if (target !== 'desktop' && target !== 'web') {
  throw new Error(`Invalid AXOME_TARGET "${target}": expected desktop or web`)
}

const routes = `src/routes-${target}`
// Targets run side by side in dev, so each keeps its own generated route manifest.
const suffix = target === 'desktop' ? '' : `-${target}`

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    files: { routes },
    outDir: `.svelte-kit${suffix}`,
    adapter: adapter({
      pages: `build${suffix}`,
      assets: `build${suffix}`,
      fallback: 'index.html'
    })
  }
}

export default config
