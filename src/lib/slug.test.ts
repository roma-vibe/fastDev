import { describe, expect, it } from 'vitest'
import { isValidId, isValidSlug, slugify, uniqueSlug } from './slug'

describe('slugify', () => {
  it('matches the Rust implementation for common names', () => {
    expect(slugify('My Shop')).toBe('my-shop')
    expect(slugify('Мой магазин')).toBe('moi-magazin')
    expect(slugify('  Hello,  World!! ')).toBe('hello-world')
    expect(slugify('Café Déjà vu')).toBe('cafe-deja-vu')
  })

  it('validates slugs and ids', () => {
    expect(isValidSlug('my-shop')).toBe(true)
    expect(isValidSlug('My Shop')).toBe(false)
    expect(isValidId('node-vue')).toBe(true)
    expect(isValidId('node--vue')).toBe(false)
    expect(isValidId('1node')).toBe(false)
  })

  it('adds a numeric suffix to a taken slug', () => {
    expect(uniqueSlug('shop', new Set())).toBe('shop')
    expect(uniqueSlug('shop', new Set(['shop', 'shop-2']))).toBe('shop-3')
    expect(uniqueSlug('a'.repeat(64), new Set(['a'.repeat(64)]))).toBe('a'.repeat(62) + '-2')
  })
})
