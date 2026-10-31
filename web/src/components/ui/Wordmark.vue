<script setup lang="ts">
defineProps<{ class?: string }>()
</script>

<template>
  <span class="font-bold leading-none text-foreground" style="letter-spacing: -0.02em" :class="$props.class">slot. <span class="font-normal text-muted-foreground">gamebit</span></span>
</template>
