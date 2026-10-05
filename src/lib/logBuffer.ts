// Log lines addressed by absolute index. Batches from events and full logs fetched in
// parallel overlap; merging by position keeps every line exactly once.

export interface LogBuffer {
  /** Absolute index of lines[0]. */
  offset: number
  lines: string[]
}

export const EMPTY_LOG: LogBuffer = { offset: 0, lines: [] }

export function mergeLog(
  current: LogBuffer | undefined,
  start: number,
  lines: string[],
  max = 5000,
): LogBuffer {
  let result: LogBuffer
  const cur = current ?? EMPTY_LOG
  const curEnd = cur.offset + cur.lines.length
  const newEnd = start + lines.length
  if (cur.lines.length === 0) {
    // Nothing kept (possibly cleared): take only lines at or after the current position.
    const skip = Math.max(0, cur.offset - start)
    result = skip >= lines.length ? cur : { offset: start + skip, lines: lines.slice(skip) }
  } else if (start > curEnd) {
    // A gap (lines were dropped on the server): the new range wins.
    result = { offset: start, lines: [...lines] }
  } else if (newEnd <= cur.offset) {
    result = cur
  } else {
    const offset = Math.min(cur.offset, start)
    const merged: string[] = new Array<string>(Math.max(curEnd, newEnd) - offset)
    cur.lines.forEach((line, i) => (merged[cur.offset - offset + i] = line))
    lines.forEach((line, i) => (merged[start - offset + i] = line))
    result = { offset, lines: merged }
  }
  if (result.lines.length > max) {
    const drop = result.lines.length - max
    result = { offset: result.offset + drop, lines: result.lines.slice(drop) }
  }
  return result
}

/** Empties the buffer but keeps its position, so later batches continue from there. */
export function clearLog(current: LogBuffer | undefined): LogBuffer {
  const cur = current ?? EMPTY_LOG
  return { offset: cur.offset + cur.lines.length, lines: [] }
}
