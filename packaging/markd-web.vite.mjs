// The suite's build of Markd's web version: Markd's own vite.config.ts with two trims for files
// the Markd tabs never load. packaging/build-markd-web.sh copies this file next to that config
// (vite resolves a config's imports from where the config file is) and removes it afterwards.
//
// - Prism's language loader imports `prism-${lang}.js`, which also matches every `prism-*.min.js`,
//   so each language was built twice; only the plain files are kept.
// - KaTeX's stylesheet offers each font as woff2, woff and ttf; the web view always takes woff2,
//   so the other two aren't built.
//
// Each trim fails the build when its source has changed, rather than quietly doing nothing.
import markd from './vite.config.ts'

const PRISM_IMPORT = /import\(\s*`\.\.\/\.\.\/\.\.\/node_modules\/prismjs\/components\/prism-\$\{lang\}\.js`,?\s*\)/
const KATEX_FALLBACKS = /,\s*url\([^)]*\.woff\) format\("woff"\),\s*url\([^)]*\.ttf\) format\("truetype"\)/g

function suiteTrims() {
  const done = new Set()
  return {
    name: 'craft-suite-trims',
    enforce: 'pre',
    transform(code, id) {
      const file = id.split('?')[0]
      if (file.endsWith('/utils/prism/loadLanguage.ts')) {
        if (!PRISM_IMPORT.test(code)) this.error('Prism loader changed: update packaging/markd-web.vite.mjs')
        done.add('prism')
        const glob = "const prismComponents = import.meta.glob(['../../../node_modules/prismjs/components/prism-*.js', '!../../../node_modules/prismjs/components/prism-*.min.js']);\n"
        return glob + code.replace(PRISM_IMPORT, 'prismComponents[`../../../node_modules/prismjs/components/prism-${lang}.js`]()')
      }
      if (/\/katex\/dist\/katex(\.min)?\.css$/.test(file)) {
        const trimmed = code.replace(KATEX_FALLBACKS, '')
        if (trimmed === code) this.error('KaTeX stylesheet changed: update packaging/markd-web.vite.mjs')
        done.add('katex')
        return trimmed
      }
    },
    buildEnd(error) {
      if (!error && (!done.has('prism') || !done.has('katex')))
        this.error(`craft-suite trims not applied (${[...done].join(', ') || 'none'}): update packaging/markd-web.vite.mjs`)
    },
  }
}

export default (env) => {
  const config = markd(env)
  return { ...config, plugins: [...(config.plugins ?? []), suiteTrims()] }
}
