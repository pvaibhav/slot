<script setup lang="ts">
defineProps<{ value: number }>()
</script>

<template>
  <div class="h-2.5 w-full overflow-hidden rounded-full bg-muted">
    <div
      class="h-full bg-primary transition-all duration-300"
      :style="{ width: `${value}%` }"
    />
  </div>
</template>
