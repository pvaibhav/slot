<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'

const props = defineProps<{
  title: string
}>()

const emit = defineEmits<{
  close: []
}>()

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close')
  }
}

onMounted(() => {
  document.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div
    class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4"
    @click.self="emit('close')"
  >
    <div class="bg-card border border-border rounded-xl p-5 w-full max-w-sm space-y-4">
      <h2 class="text-lg font-bold text-foreground">{{ props.title }}</h2>
      <slot />
      <div class="flex justify-end gap-2">
        <slot name="footer" />
      </div>
    </div>
  </div>
</template>
