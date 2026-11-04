export function saveWarning(resumeCores: string[] | undefined | null): string | null {
  if (!resumeCores || resumeCores.length === 0) return null
  return 'This game has a resume state, which loads instead of this save. Hold A when you start the game to use this save.'
}
