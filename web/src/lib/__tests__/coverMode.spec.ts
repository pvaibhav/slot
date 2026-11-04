import { describe, expect, it } from 'vitest'
import { cartAspect, MIN_PAGE_SIZE, pageSizeFor } from '../coverMode'

describe('cart shape', () => {
  it('is 360 by 212 for the GBA folders', () => {
    expect(cartAspect('GBA')).toBeCloseTo(360 / 212, 6)
    expect(cartAspect('GBAV')).toBeCloseTo(360 / 212, 6)
  })

  it('is 312 by 337 for the Game Boy folders', () => {
    expect(cartAspect('GB')).toBeCloseTo(312 / 337, 6)
    expect(cartAspect('GBC')).toBeCloseTo(312 / 337, 6)
  })
})

describe('pageSizeFor', () => {
  it('rounds the floor up to a whole row', () => {
    expect(pageSizeFor(3)).toBe(21)
    expect(pageSizeFor(4)).toBe(20)
    expect(pageSizeFor(7)).toBe(21)
  })

  it('takes at least the floor', () => {
    for (let cols = 1; cols <= 12; cols++) {
      expect(pageSizeFor(cols)).toBeGreaterThanOrEqual(MIN_PAGE_SIZE)
    }
  })

  it('treats a zero or fractional column count as one row of at least one', () => {
    expect(pageSizeFor(0)).toBe(MIN_PAGE_SIZE)
    expect(pageSizeFor(2.9)).toBe(MIN_PAGE_SIZE)
  })
})
