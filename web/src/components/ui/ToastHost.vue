<script setup lang="ts">
import { toastState } from '@/lib/toast'
import { Loader2 } from 'lucide-vue-next'
</script>

<template>
  <Transition
    enter-active-class="transition-opacity duration-200"
    enter-from-class="opacity-0"
    leave-active-class="transition-opacity duration-200"
    leave-to-class="opacity-0"
  >
    <div
      v-if="toastState.message"
      role="status"
      class="fixed bottom-6 left-1/2 z-50 flex max-w-[calc(100vw-2rem)] -translate-x-1/2 items-center gap-2.5 rounded-lg border border-border bg-card px-4 py-2.5 text-sm font-medium shadow-lg shadow-black/40"
    >
      <Loader2 v-if="toastState.working" class="h-4 w-4 shrink-0 animate-spin text-muted-foreground" />
      {{ toastState.message }}
    </div>
  </Transition>
</template>
