<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { checkSession } from '@/api/client'
import AppHeader from '@/components/layout/AppHeader.vue'
import ConfirmHost from '@/components/ui/ConfirmHost.vue'
import ToastHost from '@/components/ui/ToastHost.vue'

const route = useRoute()
const chrome = computed(() => route.name !== undefined && route.name !== 'login')

function returned() {
  if (document.visibilityState === 'visible') checkSession()
}

onMounted(() => {
  window.addEventListener('focus', returned)
  document.addEventListener('visibilitychange', returned)
})

onBeforeUnmount(() => {
  window.removeEventListener('focus', returned)
  document.removeEventListener('visibilitychange', returned)
})
</script>

<template>
  <div class="min-h-screen">
    <AppHeader v-if="chrome" />
    <RouterView />
    <ConfirmHost />
    <ToastHost />
  </div>
</template>
