<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'

const props = defineProps<{ url: string; type: string; name: string }>()

const textContent = ref('')

const ext = computed(() => props.name.slice(props.name.lastIndexOf('.') + 1).toLowerCase())

const kind = computed<'image' | 'text' | 'other'>(() => {
  const e = ext.value
  const t = props.type
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'avif'].includes(e) || t.startsWith('image/')) return 'image'
  if (['txt', 'log', 'nfo', 'json', 'csv', 'cfg', 'ini'].includes(e) || t.startsWith('text/')) return 'text'
  return 'other'
})

async function prepare() {
  textContent.value = ''
  if (kind.value === 'text') {
    textContent.value = await (await fetch(props.url)).text()
  }
}

onMounted(prepare)
watch(() => props.url, prepare)
</script>

<template>
  <div v-if="kind === 'image'" class="rounded-xl bg-surface-sunken border border-border p-4">
    <img :src="url" :alt="name" class="mx-auto max-w-full h-auto rounded" />
  </div>

  <pre
    v-else-if="kind === 'text'"
    class="rounded-xl bg-card border border-border p-5 text-sm leading-relaxed text-foreground/90 whitespace-pre-wrap break-words font-mono overflow-x-auto"
  >{{ textContent }}</pre>

  <div v-else class="py-16 text-center">
    <p class="text-muted-foreground">{{ $t('browse.preview.notAvailable') }}</p>
  </div>
</template>
