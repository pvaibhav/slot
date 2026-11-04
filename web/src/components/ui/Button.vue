<script setup lang="ts">
import { cn } from '@/lib/utils'
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  variant?: 'default' | 'outline' | 'ghost' | 'destructive'
  size?: 'default' | 'sm' | 'lg' | 'icon'
  disabled?: boolean
}>(), {
  variant: 'default',
  size: 'default',
})

const classes = computed(() =>
  cn(
    'inline-flex items-center justify-center gap-2 rounded-lg text-sm font-medium transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50 cursor-pointer',
    {
      'bg-accent text-accent-foreground shadow-sm hover:bg-accent-light active:scale-[0.97]': props.variant === 'default',
      'border border-border bg-transparent hover:bg-muted hover:border-muted-foreground/30 active:scale-[0.97]': props.variant === 'outline',
      'hover:bg-muted text-muted-foreground hover:text-foreground': props.variant === 'ghost',
      'bg-destructive text-foreground shadow-sm hover:bg-destructive/90 active:scale-[0.97]': props.variant === 'destructive',
    },
    {
      'h-10 px-5 py-2': props.size === 'default',
      'h-8 px-3 text-xs': props.size === 'sm',
      'h-12 px-6 text-base': props.size === 'lg',
      'h-9 w-9 rounded-lg': props.size === 'icon',
    },
  ),
)
</script>

<template>
  <button :class="classes" :disabled="disabled">
    <slot />
  </button>
</template>
