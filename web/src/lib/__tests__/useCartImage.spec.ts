import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope, nextTick, ref } from 'vue'

const imageBlob = vi.hoisted(() => vi.fn())
vi.mock('@/api/client', () => ({ imageBlob }))

import { RETRY_MS, useCartImage } from '@/composables/useCartImage'
import { enqueue } from '@/lib/imageQueue'

let observers: ((entries: { isIntersecting: boolean }[]) => void)[] = []
const revoked = vi.fn()

function mount(url: { value: string }) {
  const scope = effectScope()
  const src = scope.run(() => useCartImage(() => url.value, ref({} as Element)))!
  return { src, stop: () => scope.stop() }
}

function show() {
  observers.forEach(cb => cb([{ isIntersecting: true }]))
}

async function settle(ms = 0) {
  await vi.advanceTimersByTimeAsync(ms)
}

beforeEach(() => {
  vi.useFakeTimers()
  observers = []
  imageBlob.mockReset()
  revoked.mockReset()
  vi.stubGlobal('IntersectionObserver', class {
    constructor(cb: (entries: { isIntersecting: boolean }[]) => void) {
      observers.push(cb)
    }
    observe() {}
    disconnect() {}
  })
  URL.revokeObjectURL = revoked
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('useCartImage', () => {
  it('waits until the cart is near the view', async () => {
    imageBlob.mockResolvedValue('blob:one')
    const view = mount(ref('/cart?v=1'))
    await settle()
    expect(imageBlob).not.toHaveBeenCalled()
    show()
    await settle()
    expect(view.src.value).toBe('blob:one')
    view.stop()
  })

  it('retries a failed fetch with backoff', async () => {
    imageBlob.mockRejectedValueOnce(new Error('reset')).mockResolvedValueOnce(null).mockResolvedValue('blob:one')
    const view = mount(ref('/cart?v=1'))
    show()
    await settle()
    expect(imageBlob).toHaveBeenCalledTimes(1)
    await settle(RETRY_MS[0]! - 1)
    expect(imageBlob).toHaveBeenCalledTimes(1)
    await settle(1)
    expect(imageBlob).toHaveBeenCalledTimes(2)
    await settle(RETRY_MS[1]!)
    expect(imageBlob).toHaveBeenCalledTimes(3)
    expect(view.src.value).toBe('blob:one')
    view.stop()
  })

  it('stops after the last retry and loads the url again when it comes back', async () => {
    imageBlob.mockResolvedValue(null)
    const url = ref('/cart?v=1')
    const view = mount(url)
    show()
    await settle(RETRY_MS.reduce((a, b) => a + b, 0))
    expect(imageBlob).toHaveBeenCalledTimes(RETRY_MS.length + 1)
    await settle(60000)
    expect(imageBlob).toHaveBeenCalledTimes(RETRY_MS.length + 1)
    imageBlob.mockResolvedValue('blob:two')
    url.value = '/cart?v=2'
    await nextTick()
    await settle()
    url.value = '/cart?v=1'
    await nextTick()
    await settle()
    expect(imageBlob).toHaveBeenLastCalledWith('/cart?v=1', expect.anything())
    expect(view.src.value).toBe('blob:two')
    view.stop()
  })

  it('revokes the old blob on a new url and the last on unmount', async () => {
    imageBlob.mockResolvedValueOnce('blob:one').mockResolvedValueOnce('blob:two')
    const url = ref('/cart?v=1')
    const view = mount(url)
    show()
    await settle()
    url.value = '/cart?v=2'
    await nextTick()
    await settle()
    expect(view.src.value).toBe('blob:two')
    expect(revoked).toHaveBeenCalledWith('blob:one')
    view.stop()
    expect(revoked).toHaveBeenCalledWith('blob:two')
  })

  it('aborts the fetch on unmount and drops what arrives after', async () => {
    let finish!: (blob: string) => void
    let signal!: AbortSignal
    imageBlob.mockImplementation((_: string, s: AbortSignal) => {
      signal = s
      return new Promise(resolve => { finish = resolve })
    })
    const view = mount(ref('/cart?v=1'))
    show()
    await settle()
    view.stop()
    expect(signal.aborted).toBe(true)
    finish('blob:late')
    await settle()
    expect(view.src.value).toBeNull()
    expect(revoked).toHaveBeenCalledWith('blob:late')
  })
})

describe('imageQueue', () => {
  it('runs three at a time and skips an aborted waiter', async () => {
    const releases: (() => void)[] = []
    const task = vi.fn(() => new Promise<void>(resolve => { releases.push(resolve) }))
    const open = new AbortController()
    const leaving = new AbortController()
    const jobs = [0, 1, 2].map(() => enqueue(task, open.signal))
    const gone = enqueue(task, leaving.signal)
    const last = enqueue(task, open.signal)
    expect(task).toHaveBeenCalledTimes(3)
    leaving.abort()
    await expect(gone).rejects.toThrow('Aborted')
    releases[0]!()
    await settle()
    expect(task).toHaveBeenCalledTimes(4)
    releases.slice(1).forEach(r => r())
    await settle()
    await Promise.all([...jobs, last])
    expect(task).toHaveBeenCalledTimes(4)
  })
})
