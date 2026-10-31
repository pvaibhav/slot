export function pinFromSearch(search: string): string | null {
  const raw = new URLSearchParams(search).get('pin')
  if (!raw) return null
  const pin = raw.trim()
  return pin.length > 0 ? pin : null
}

export const PIN_LENGTH = 4

export function normalizePin(raw: string): string[] {
  const chars = raw.replace(/\D/g, '').slice(0, PIN_LENGTH).split('').filter(Boolean)
  return [...chars, ...Array(PIN_LENGTH - chars.length).fill('')]
}

export function pinComplete(digits: string[]): boolean {
  return digits.length === PIN_LENGTH && digits.every(d => d !== '')
}
