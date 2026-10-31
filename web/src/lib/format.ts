const SIZE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB']

export function formatSize(bytes: number): string {
  if (!bytes || bytes <= 0) return ''
  let v = bytes
  let u = 0
  while (v >= 1024 && u < SIZE_UNITS.length - 1) {
    v /= 1024
    u++
  }
  return `${v.toFixed(v >= 10 || u === 0 ? 0 : 1)} ${SIZE_UNITS[u]}`
}

export function formatRelativeTime(ms: number): string {
  if (!ms || ms <= 0) return ''
  const min = Math.floor((Date.now() - ms) / 60000)
  if (min < 1) return 'just now'
  if (min < 60) return `${min}m ago`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr}h ago`
  return `${Math.floor(hr / 24)}d ago`
}

export function stripExtension(name: string): string {
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(0, i) : name
}

export function fileExt(name: string): string {
  const i = name.lastIndexOf('.')
  return i >= 0 ? name.slice(i + 1).toLowerCase() : ''
}

const PREVIEWABLE = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'avif', 'txt', 'log', 'nfo', 'json', 'csv', 'cfg', 'ini']

export function isPreviewable(name: string): boolean {
  return PREVIEWABLE.includes(fileExt(name))
}
