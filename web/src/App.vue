<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import AppHeader from '@/components/layout/AppHeader.vue'
import ConfirmHost from '@/components/ui/ConfirmHost.vue'
import ToastHost from '@/components/ui/ToastHost.vue'

const route = useRoute()
const chrome = computed(() => route.name !== undefined && route.name !== 'login')
</script>

<template>
  <div class="min-h-screen">
    <AppHeader v-if="chrome" />
    <RouterView />
    <ConfirmHost />
    <ToastHost />
  </div>
</template>
