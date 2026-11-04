import { reactive } from 'vue'

export interface ConfirmOptions {
  title: string
  body?: string
  confirmLabel?: string
  cancelLabel?: string
  destructive?: boolean
}

interface PendingConfirm extends ConfirmOptions {
  resolve: (ok: boolean) => void
}

export const confirmState = reactive<{ pending: PendingConfirm | null }>({ pending: null })

export function confirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise(resolve => {
    confirmState.pending = { ...options, resolve }
  })
}

export function resolveConfirm(ok: boolean) {
  const p = confirmState.pending
  confirmState.pending = null
  p?.resolve(ok)
}
