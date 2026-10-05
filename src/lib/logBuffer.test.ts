import { describe, expect, it } from 'vitest'
import { clearLog, mergeLog } from './logBuffer'

describe('mergeLog', () => {
  it('appends consecutive batches', () => {
    let log = mergeLog(undefined, 0, ['a', 'b'])
    log = mergeLog(log, 2, ['c'])
    expect(log).toEqual({ offset: 0, lines: ['a', 'b', 'c'] })
  })

  it('does not duplicate overlapping ranges', () => {
    let log = mergeLog(undefined, 2, ['c', 'd'])
    log = mergeLog(log, 0, ['a', 'b', 'c'])
    log = mergeLog(log, 3, ['d', 'e'])
    expect(log.lines).toEqual(['a', 'b', 'c', 'd', 'e'])
  })

  it('caps the size and continues after clearing', () => {
    let log = mergeLog(undefined, 0, ['1', '2', '3', '4'], 3)
    expect(log).toEqual({ offset: 1, lines: ['2', '3', '4'] })
    log = clearLog(log)
    log = mergeLog(log, 3, ['4'])
    expect(log.lines).toEqual([])
    log = mergeLog(log, 4, ['5'])
    expect(log).toEqual({ offset: 4, lines: ['5'] })
  })
})
