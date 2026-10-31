<script setup lang="ts">
import { computed, ref } from 'vue'
import type { GameRow } from '@/api/client'
import { useCartImage } from '@/composables/useCartImage'
import { cartAspect } from '@/lib/coverMode'

const props = defineProps<{
  tag: string
  game: GameRow
  title?: string
}>()

const root = ref<HTMLElement | null>(null)
const src = useCartImage(() => props.game.cartUrl, root)
const shape = computed(() => ({ aspectRatio: String(cartAspect(props.tag)) }))
</script>

<template>
  <div ref="root">
    <div class="flex h-40 items-center justify-center p-2">
      <img
        v-if="src"
        :src="src"
        :alt="game.displayName"
        loading="lazy"
        decoding="async"
        class="max-h-full max-w-full object-contain"
        :style="shape"
      />
      <div v-else class="max-h-full max-w-full rounded bg-white/5 h-full" :style="shape"></div>
    </div>
    <div class="flex h-[2.625rem] items-center justify-center px-2 text-center">
      <span class="line-clamp-2 text-xs font-bold leading-tight text-foreground">{{ title ?? game.displayName }}</span>
    </div>
  </div>
</template>
