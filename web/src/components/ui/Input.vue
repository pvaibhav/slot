<script setup lang="ts">
import { cn } from '@/lib/utils'

defineProps<{
  class?: string
  modelValue?: string
  type?: string
  placeholder?: string
}>()

defineEmits<{ 'update:modelValue': [value: string] }>()
</script>

<template>
  <input
    :type="type ?? 'text'"
    :value="modelValue"
    :placeholder="placeholder"
    :class="cn(
      'flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
      $props.class,
    )"
    @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
  />
</template>
