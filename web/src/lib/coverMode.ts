const GBA_CART = 360 / 212
const GB_CART = 312 / 337

export function cartAspect(tag: string): number {
  return tag === 'GB' || tag === 'GBC' ? GB_CART : GBA_CART
}

export const MIN_PAGE_SIZE = 20

export function pageSizeFor(cols: number, min: number = MIN_PAGE_SIZE): number {
  const c = Math.max(1, Math.floor(cols))
  const floor = Math.max(1, Math.floor(min))
  return c * Math.ceil(floor / c)
}
