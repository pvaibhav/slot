<script setup lang="ts">
import { cn } from '@/lib/utils'

defineProps<{ class?: string }>()
</script>

<template>
  <div :class="cn('rounded-xl border border-border bg-card p-6 transition-all duration-150', $props.class)">
    <slot />
  </div>
</template>
