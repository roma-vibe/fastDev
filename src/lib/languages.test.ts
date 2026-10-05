import { describe, expect, it } from 'vitest'
import { language } from './languages'

describe('language', () => {
  it('uses the display name of a known language', () => {
    expect(language('node').name).toBe('Node.js')
    expect(language('dart')).toMatchObject({ name: 'Dart', short: 'DA' })
  })

  it('capitalises languages without an entry', () => {
    expect(language('kotlin')).toMatchObject({ name: 'Kotlin', short: 'KO' })
  })
})
