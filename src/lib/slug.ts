// Client-side slug preview; mirrors the Rust `slugify` (deunicode transliteration) for common scripts.

const CYRILLIC: Record<string, string> = {
  а: 'a',
  б: 'b',
  в: 'v',
  г: 'g',
  д: 'd',
  е: 'e',
  ё: 'e',
  ж: 'zh',
  з: 'z',
  и: 'i',
  й: 'i',
  к: 'k',
  л: 'l',
  м: 'm',
  н: 'n',
  о: 'o',
  п: 'p',
  р: 'r',
  с: 's',
  т: 't',
  у: 'u',
  ф: 'f',
  х: 'kh',
  ц: 'ts',
  ч: 'ch',
  ш: 'sh',
  щ: 'shch',
  ъ: '',
  ы: 'y',
  ь: '',
  э: 'e',
  ю: 'iu',
  я: 'ia',
  і: 'i',
  ї: 'i',
  є: 'ie',
  ґ: 'g',
}

export function slugify(name: string): string {
  const transliterated = [...name.toLowerCase()].map((c) => CYRILLIC[c] ?? c).join('')
  return transliterated
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64)
    .replace(/-+$/g, '')
}

export function isValidSlug(slug: string): boolean {
  return /^[a-z0-9][a-z0-9._-]{0,63}$/.test(slug)
}

export function isValidId(id: string): boolean {
  return /^[a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,63}$/.test(id)
}
