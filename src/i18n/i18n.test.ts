import { describe, expect, it } from 'vitest'
import { locale, russianPlural, st, t, tn } from './index'

describe('i18n', () => {
  it('interpolates parameters', () => {
    locale.value = 'en'
    expect(t('Hello, {name}', { name: 'Roma' })).toBe('Hello, Roma')
    expect(tn(1, '{n} project', '{n} projects')).toBe('1 project')
    expect(tn(3, '{n} project', '{n} projects')).toBe('3 projects')
  })

  it('translates skeleton texts with their own dictionary first', () => {
    const own = { ru: { 'Everything in Docker': 'Полностью в Docker', Vue: 'Vue' } }
    locale.value = 'ru'
    expect(st('Everything in Docker', own)).toBe('Полностью в Docker')
    expect(st('No Docker', own)).toBe('Без Docker')
    expect(st('Laravel + Inertia', own)).toBe('Laravel + Inertia')
    expect(st(undefined, own)).toBe('')
    locale.value = 'en'
    expect(st('Everything in Docker', own)).toBe('Everything in Docker')
  })

  it('picks Russian plural forms', () => {
    expect([1, 2, 5, 11, 21, 22, 25, 111].map(russianPlural)).toEqual([0, 1, 2, 2, 0, 1, 2, 2])
  })
})
