import { reactive } from 'vue'

const SHOWN_MS = 3500

export const toastState = reactive<{ message: string | null; working: boolean }>({ message: null, working: false })

let timer: ReturnType<typeof setTimeout> | undefined

export function toast(message: string, working = false) {
  clearTimeout(timer)
  toastState.message = message
  toastState.working = working
  if (!working) timer = setTimeout(() => { toastState.message = null }, SHOWN_MS)
}
