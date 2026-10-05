// Fails when a string passed to t() / tn() in src/ has no Russian translation in src/i18n/ru.ts.
// Usage: node scripts/i18n-check.ts [--list]  (--list prints missing keys as a TS snippet)

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import ru from '../src/i18n/ru.ts'

const root = new URL('../src/', import.meta.url).pathname
const files: string[] = []
const walk = (dir: string): void => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) walk(path)
    else if (
      /\.(vue|ts)$/.test(name) &&
      !name.endsWith('.test.ts') &&
      !path.includes('/i18n/') &&
      !path.endsWith('mock.ts')
    )
      files.push(path)
  }
}
walk(root)

const literal = String.raw`'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)"`
const tCall = new RegExp(String.raw`\bt\(\s*(?:${literal})`, 'g')
const tnCall = new RegExp(String.raw`\btn\(\s*[^,]+,\s*(?:${literal})\s*,\s*(?:${literal})`, 'g')
const unescape = (s: string): string => s.replace(/\\(['"\\])/g, '$1')

const missing = new Map<string, 'string' | 'plural'>()
for (const file of files) {
  const text = readFileSync(file, 'utf8')
  for (const m of text.matchAll(tCall)) {
    const key = unescape(m[1] ?? m[2] ?? '')
    if (typeof ru[key] !== 'string') missing.set(key, 'string')
  }
  for (const m of text.matchAll(tnCall)) {
    const key = unescape(m[3] ?? m[4] ?? '')
    if (!Array.isArray(ru[key])) missing.set(key, 'plural')
  }
}

if (missing.size === 0) {
  console.log('i18n: all strings have Russian translations')
} else {
  if (process.argv.includes('--list')) {
    for (const [key, kind] of missing) {
      console.log(
        kind === 'plural' ? `  ${JSON.stringify(key)}: ['', '', ''],` : `  ${JSON.stringify(key)}: '',`,
      )
    }
  } else {
    console.error(`i18n: ${missing.size} string(s) without Russian translation:`)
    for (const key of missing.keys()) console.error(`  - ${key}`)
    console.error('Run `node scripts/i18n-check.ts --list` for a snippet to paste into src/i18n/ru.ts.')
  }
  process.exit(1)
}
