<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { HOME_TAGS, platformIcon, platformName } from '@/api/platforms'
import { Wallpaper, Power } from 'lucide-vue-next'
import ToolsTab from '@/components/tools/ToolsTab.vue'

type Tab = 'content' | 'customization' | 'tools'

const props = defineProps<{ tab?: string }>()
const router = useRouter()
const activeTab = computed<Tab>(() => {
  if (props.tab === 'customization') return 'customization'
  if (props.tab === 'tools') return 'tools'
  return 'content'
})

const VALID_TABS = ['games', 'customization', 'tools']
watch(() => props.tab, t => {
  if (t && !VALID_TABS.includes(t)) router.replace('/dashboard/games')
}, { immediate: true })

function openPlatform(tag: string) {
  router.push({ name: 'platform', params: { tag } })
}

function browseFlat(resource: string) {
  router.push({ name: 'browse-flat', params: { resource } })
}

const cardClass = 'text-left cursor-pointer rounded-xl border border-border bg-card hover:ring-2 hover:ring-primary hover:ring-offset-2 hover:ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background'
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6 pb-0 space-y-6 min-h-[calc(100vh-61px)] flex flex-col">
    <template v-if="activeTab === 'content'">
      <div class="flex-1 grid grid-cols-1 sm:grid-cols-3 gap-4 content-start">
        <button
          v-for="tag in HOME_TAGS"
          :key="tag"
          type="button"
          :class="[cardClass, 'flex flex-col items-center gap-5 px-6 py-8 sm:py-14']"
          @click="openPlatform(tag)"
        >
          <img :src="platformIcon(tag)" :alt="platformName(tag)" class="h-32 sm:h-48 w-full object-contain" />
          <span class="slot-label text-sm text-center">{{ platformName(tag) }}</span>
        </button>
      </div>
    </template>

    <template v-if="activeTab === 'tools'">
      <ToolsTab class="flex-1" />
    </template>

    <template v-if="activeTab === 'customization'">
      <div class="space-y-4 flex-1">
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <button type="button" :class="[cardClass, 'p-5']" @click="browseFlat('wallpapers')">
            <div class="flex items-center gap-3">
              <div class="flex items-center justify-center h-10 w-10 rounded-lg bg-muted">
                <Wallpaper class="h-5 w-5 text-accent" />
              </div>
              <span class="font-semibold">{{ $t('dashboard.wallpapers') }}</span>
            </div>
          </button>
          <button type="button" :class="[cardClass, 'p-5']" @click="router.push({ name: 'boot-logo' })">
            <div class="flex items-center gap-3">
              <div class="flex items-center justify-center h-10 w-10 rounded-lg bg-muted">
                <Power class="h-5 w-5 text-accent" />
              </div>
              <span class="font-semibold">{{ $t('dashboard.bootLogo') }}</span>
            </div>
          </button>
        </div>
      </div>
    </template>


    <footer class="pt-8 pb-6 text-center text-xs text-muted-foreground/60 border-t border-border/50">
      <span>{{ $t('dashboard.footerIconsPrefix') }} </span><a href="https://git.libretro.com/libretro-assets/retroarch-assets" class="underline hover:text-muted-foreground" target="_blank" rel="noopener">Libretro</a><span> {{ $t('dashboard.footerIconsSuffix') }}</span><br />
      {{ $t('dashboard.footerTrademarks') }}
    </footer>
  </div>
</template>
