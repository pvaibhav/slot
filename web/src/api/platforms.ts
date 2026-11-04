import type { LocationQueryRaw } from 'vue-router'

export const PLATFORM_TAGS = ['GB', 'GBC', 'GBA', 'GBAV'] as const

export const PLATFORM_NAMES: Record<string, string> = {
  GBA: 'Game Boy Advance',
  GBAV: 'Game Boy Advance Video',
  GB: 'Game Boy',
  GBC: 'Game Boy Color',
}

export const HOME_TAGS = ['GB', 'GBC', 'GBA'] as const

export const VIDEO_TAG = 'GBAV'
export const VIDEO_HOME = 'GBA'

export function platformRoute(tag: string, query: LocationQueryRaw = {}) {
  return tag === VIDEO_TAG
    ? { name: 'platform-video', params: { tag: VIDEO_HOME }, query }
    : { name: 'platform', params: { tag }, query }
}

export function biosFile(tag: string): string {
  return `${tag.toLowerCase()}_bios.bin`
}

export function platformIcon(tag: string): string {
  return `/platforms/${tag}.svg`
}

export function platformName(tag: string): string {
  return PLATFORM_NAMES[tag] ?? tag
}
