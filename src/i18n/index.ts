// Minimal i18n: English source strings in code, Russian dictionary in ru.ts.
// `t('Create project')`, `t('Hello, {name}', { name })`, `tn(count, '{n} project', '{n} projects')`.

import { ref } from 'vue'
import ru from './ru'

export type Locale = 'en' | 'ru'

export const locale = ref<Locale>('en')

type Params = Record<string, string | number>

function interpolate(text: string, params?: Params): string {
  if (!params) return text
  return text.replace(/\{(\w+)\}/g, (match, key: string) => (key in params ? String(params[key]) : match))
}

export function t(text: string, params?: Params): string {
  if (locale.value === 'ru') {
    const entry = ru[text]
    if (typeof entry === 'string') return interpolate(entry, params)
  }
  return interpolate(text, params)
}

/** Translations shipped by a skeleton (`[translations.<lang>]`): language → English text → translation. */
export type Translations = Record<string, Record<string, string>>

/**
 * A text written by a skeleton author (name, description, option or command label…): the
 * skeleton's own translation, else the app dictionary (common labels such as "No Docker"),
 * else the English text.
 */
export function st(text: string | null | undefined, translations?: Translations | null): string {
  if (!text) return ''
  if (locale.value !== 'en') {
    const own = translations?.[locale.value]?.[text]
    if (own) return own
  }
  return t(text)
}

/** Plural form: English "one/other", Russian "one/few/many" from the dictionary entry of `other`. */
export function tn(count: number, one: string, other: string, params?: Params): string {
  const all = { n: count, ...params }
  if (locale.value === 'ru') {
    const entry = ru[other]
    if (Array.isArray(entry)) return interpolate(entry[russianPlural(count)] ?? other, all)
  }
  return interpolate(count === 1 ? one : other, all)
}

export function russianPlural(count: number): 0 | 1 | 2 {
  const n = Math.abs(count) % 100
  const last = n % 10
  if (n > 10 && n < 20) return 2
  if (last === 1) return 0
  if (last >= 2 && last <= 4) return 1
  return 2
}

export function detectLocale(): Locale {
  return navigator.language.toLowerCase().startsWith('ru') ? 'ru' : 'en'
}

export function formatDate(value: string | null | undefined): string {
  if (!value) return ''
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleDateString(locale.value === 'ru' ? 'ru-RU' : 'en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}

export function formatRelative(value: string | null | undefined): string {
  if (!value) return ''
  const date = new Date(value)
  const seconds = Math.round((date.getTime() - Date.now()) / 1000)
  const rtf = new Intl.RelativeTimeFormat(locale.value, { numeric: 'auto' })
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['day', 86400],
    ['hour', 3600],
    ['minute', 60],
  ]
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return rtf.format(Math.round(seconds / size), unit)
  }
  return rtf.format(seconds, 'second')
}
