<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router'
import { ChevronRight } from 'lucide-vue-next'

export interface Crumb {
  label: string
  to?: RouteLocationRaw
}

defineProps<{ items: Crumb[] }>()
</script>

<template>
  <nav class="flex items-center gap-2 text-base sm:text-lg min-w-0" :aria-label="$t('dialogs.breadcrumb')">
    <template v-for="(item, i) in items" :key="i">
      <ChevronRight v-if="i > 0" class="h-4 w-4 shrink-0 text-muted-foreground/40" :class="{ 'hidden sm:block': i < items.length - 1 }" />
      <RouterLink
        v-if="item.to && i < items.length - 1"
        :to="item.to"
        class="text-muted-foreground hover:text-foreground transition-colors truncate min-w-0 sm:shrink-0"
        :class="{ 'hidden sm:block': i < items.length - 2 }"
      >{{ item.label }}</RouterLink>
      <span
        v-else
        class="text-foreground font-semibold truncate min-w-0"
        :aria-current="i === items.length - 1 ? 'page' : undefined"
      >{{ item.label }}</span>
    </template>
  </nav>
</template>
