import { describe, expect, it } from 'vitest'
import { bareTitle, shownTitles } from '@/lib/names'

describe('names', () => {
  it('drops tags', () => {
    expect(bareTitle("Kirby's Pinball Land (USA, Europe)")).toBe("Kirby's Pinball Land")
    expect(bareTitle('Pokemon Picross (Japan) (Proto) [!]')).toBe('Pokemon Picross')
  })

  it('keeps a name that is only tags', () => {
    expect(bareTitle('(Unknown)')).toBe('(Unknown)')
  })

  it('keeps tags only where two games would look the same', () => {
    const shown = shownTitles(['Tetris (USA)', 'Tetris (Japan)', 'Metroid (USA)'])
    expect(shown.get('Tetris (USA)')).toBe('Tetris (USA)')
    expect(shown.get('Tetris (Japan)')).toBe('Tetris (Japan)')
    expect(shown.get('Metroid (USA)')).toBe('Metroid')
  })
})
