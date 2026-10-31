<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { clearCredentials } from '@/api/client'
import Wordmark from '@/components/ui/Wordmark.vue'
import { LogOut } from 'lucide-vue-next'

const NAV = [
  { tab: 'games', label: 'dashboard.tabPlatforms' },
  { tab: 'customization', label: 'dashboard.tabCustomization' },
  { tab: 'tools', label: 'dashboard.tabTools' },
]

const route = useRoute()
const router = useRouter()

const current = computed(() => {
  if (route.name === 'dashboard') return (route.params.tab as string) || 'games'
  if (route.name === 'boot-logo' || route.params.resource === 'wallpapers') return 'customization'
  if (route.name === 'tools-files' || route.params.resource === 'fs') return 'tools'
  return 'games'
})

function disconnect() {
  clearCredentials()
  router.replace({ name: 'login' })
}

function goHome() {
  router.push({ name: 'dashboard' })
}
</script>

<template>
  <header class="sticky top-0 z-40 border-b border-border bg-background/90 backdrop-blur-md">
    <div class="grid grid-cols-[1fr_auto] min-[880px]:grid-cols-[1fr_auto_1fr] items-center gap-x-8 gap-y-1 min-h-[60px] px-4 min-[880px]:px-8 py-2 min-[880px]:py-0">
      <button
        type="button"
        class="justify-self-start rounded-lg cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
        @click="goHome"
      >
        <Wordmark class="text-[22px]" />
      </button>
      <nav class="order-last col-span-2 min-[880px]:order-none min-[880px]:col-span-1 flex gap-0.5 overflow-x-auto">
        <RouterLink
          v-for="item in NAV"
          :key="item.tab"
          :to="{ name: 'dashboard', params: { tab: item.tab } }"
          class="block rounded-lg px-3.5 py-2 text-[16px] font-semibold whitespace-nowrap transition-colors"
          :class="current === item.tab ? 'bg-border text-foreground' : 'text-muted-foreground hover:text-foreground'"
          :aria-current="current === item.tab ? 'page' : undefined"
        >
          {{ $t(item.label) }}
        </RouterLink>
      </nav>
      <button
        type="button"
        class="justify-self-end grid place-items-center cursor-pointer text-muted-foreground transition-colors hover:text-foreground"
        :aria-label="$t('common.disconnect')"
        :title="$t('common.disconnect')"
        @click="disconnect"
      >
        <LogOut class="h-6 w-6" />
      </button>
    </div>
  </header>
</template>
