import { onScopeDispose, ref, watch, type Ref } from 'vue'
import { imageBlob } from '@/api/client'
import { enqueue } from '@/lib/imageQueue'

const NEAR_VIEW = '200px'
export const RETRY_MS = [500, 1500, 4000]

function pause(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise(resolve => {
    const timer = setTimeout(resolve, ms)
    signal.addEventListener('abort', () => {
      clearTimeout(timer)
      resolve()
    }, { once: true })
  })
}

export function useCartImage(url: () => string | undefined, target: Ref<Element | null>) {
  const src = ref<string | null>(null)
  let visible = false
  let wanted: string | undefined
  let control: AbortController | null = null
  let observer: IntersectionObserver | null = null

  async function load() {
    const path = url()
    if (!visible || !path || path === wanted) return
    control?.abort()
    const current = new AbortController()
    control = current
    wanted = path
    for (let attempt = 0; ; attempt++) {
      const blob = await enqueue(() => imageBlob(path, current.signal), current.signal).catch(() => null)
      if (current.signal.aborted) {
        if (blob) URL.revokeObjectURL(blob)
        return
      }
      if (blob) {
        if (src.value) URL.revokeObjectURL(src.value)
        src.value = blob
        return
      }
      const delay = RETRY_MS[attempt]
      if (delay === undefined) {
        wanted = undefined
        return
      }
      await pause(delay, current.signal)
      if (current.signal.aborted) return
    }
  }

  watch(target, el => {
    observer?.disconnect()
    if (!el || visible) return
    observer = new IntersectionObserver(entries => {
      if (!entries.some(e => e.isIntersecting)) return
      observer?.disconnect()
      visible = true
      load()
    }, { rootMargin: NEAR_VIEW })
    observer.observe(el)
  }, { immediate: true })

  watch(url, load)

  onScopeDispose(() => {
    control?.abort()
    observer?.disconnect()
    if (src.value) URL.revokeObjectURL(src.value)
  })

  return src
}
