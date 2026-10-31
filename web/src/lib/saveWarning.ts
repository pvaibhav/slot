export function saveWarning(resumeCores: string[] | undefined | null): string | null {
  if (!resumeCores || resumeCores.length === 0) return null
  return 'This game has a suspended session, which will load over this save. Hold A when you start it next.'
}
