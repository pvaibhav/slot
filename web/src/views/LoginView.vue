<script setup lang="ts">
import { ref, onMounted, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { setCredentials, clearCredentials, getTags, PIN_KEY } from '@/api/client'
import { pinFromSearch, normalizePin, pinComplete, PIN_LENGTH } from '@/lib/pin'
import Card from '@/components/ui/Card.vue'
import Wordmark from '@/components/ui/Wordmark.vue'

const router = useRouter()
const route = useRoute()

const redirectTarget = (() => {
  const r = route.query.redirect
  return typeof r === 'string' && r.startsWith('/') && r !== '/' ? r : null
})()

function proceed() {
  router.push(redirectTarget ?? { name: 'dashboard' })
}

const digits = ref<string[]>(Array(PIN_LENGTH).fill(''))
const digitRefs = ref<HTMLInputElement[]>([])
const pinError = ref(false)
const connectionError = ref(false)
const loading = ref(false)
const checking = ref(true)

onMounted(async () => {
  if (route.query.error === 'connection') {
    connectionError.value = true
  }

  const queryPin = pinFromSearch(window.location.search)
  if (queryPin || window.location.search) {
    window.history.replaceState({}, '', window.location.pathname)
  }

  const pin = queryPin ?? localStorage.getItem(PIN_KEY)
  if (pin) {
    digits.value = normalizePin(pin)
    if (pinComplete(digits.value)) {
      await connect(queryPin === null)
      return
    }
  }

  checking.value = false
  nextTick(() => digitRefs.value[0]?.focus())
})

function setDigitRef(el: unknown, i: number) {
  if (el instanceof HTMLInputElement) digitRefs.value[i] = el
}

async function connect(silent = false) {
  const pin = digits.value.join('')
  if (!pinComplete(digits.value)) return

  connectionError.value = false
  loading.value = true
  try {
    setCredentials(pin)
    await getTags()
    proceed()
  } catch (err) {
    clearCredentials()
    digits.value = Array(PIN_LENGTH).fill('')
    loading.value = false
    checking.value = false
    nextTick(() => digitRefs.value[0]?.focus())
    if (err instanceof Error) {
      if (err.message === 'ConnectionError') {
        connectionError.value = true
      } else if (!silent && err.message === 'Unauthorized') {
        pinError.value = true
        setTimeout(() => { pinError.value = false }, 600)
      }
    }
  }
}

function onDigitInput(i: number, event: Event) {
  const input = event.target as HTMLInputElement
  const val = input.value.replace(/\D/g, '')

  if (val.length > 1) {
    const chars = val.slice(0, PIN_LENGTH - i).split('')
    chars.forEach((ch, offset) => {
      digits.value[i + offset] = ch
    })
    const next = Math.min(i + chars.length, PIN_LENGTH - 1)
    digitRefs.value[next]?.focus()
  } else {
    digits.value[i] = val
    if (val && i < PIN_LENGTH - 1) digitRefs.value[i + 1]?.focus()
  }

  if (pinComplete(digits.value)) connect()
}

function onDigitKeydown(i: number, event: KeyboardEvent) {
  if (event.key === 'Backspace' && !digits.value[i] && i > 0) {
    digits.value[i - 1] = ''
    digitRefs.value[i - 1]?.focus()
  }
}

function onDigitPaste(event: ClipboardEvent) {
  event.preventDefault()
  const chars = normalizePin(event.clipboardData?.getData('text') ?? '').filter(Boolean)
  chars.forEach((ch, i) => { digits.value[i] = ch })
  const next = Math.min(chars.length, PIN_LENGTH - 1)
  digitRefs.value[next]?.focus()
  if (pinComplete(digits.value)) connect()
}
</script>

<template>
  <div class="flex min-h-screen items-center justify-center p-4">
    <Card class="w-full max-w-sm space-y-6">
      <div class="space-y-3 text-center">
        <Wordmark class="text-5xl" />
        <template v-if="!checking">
          <p v-if="connectionError" class="text-sm font-medium text-destructive">{{ $t('login.cannotConnect') }}<br>{{ $t('login.ensureRunning') }}</p>
          <p v-else class="text-lg font-semibold text-foreground">{{ $t('login.enterPin') }}</p>
        </template>
      </div>

      <div v-if="!checking" class="space-y-2">
        <div class="flex justify-center gap-2" :class="{ 'animate-shake': pinError }" @paste="onDigitPaste">
          <input
            v-for="(_, i) in PIN_LENGTH"
            :key="i"
            :ref="(el) => setDigitRef(el, i)"
            type="text"
            inputmode="numeric"
            maxlength="2"
            :value="digits[i]"
            :disabled="loading"
            class="h-12 w-12 rounded-lg border bg-background text-center text-lg font-mono font-bold text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50 transition-colors"
            :class="pinError ? 'border-destructive' : 'border-input'"
            @input="onDigitInput(i, $event)"
            @keydown="onDigitKeydown(i, $event)"
          />
        </div>
      </div>
    </Card>
  </div>
</template>

<style scoped>
.animate-shake {
  animation: shake 0.4s ease-in-out;
}
@keyframes shake {
  0%, 100% { transform: translateX(0); }
  20% { transform: translateX(-6px); }
  40% { transform: translateX(6px); }
  60% { transform: translateX(-4px); }
  80% { transform: translateX(4px); }
}
</style>
