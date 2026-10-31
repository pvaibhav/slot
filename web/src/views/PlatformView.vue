<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { platformName, VIDEO_HOME, VIDEO_TAG } from '@/api/platforms'
import Dropdown from '@/components/ui/Dropdown.vue'
import type { DropdownItem } from '@/components/ui/Dropdown.vue'
import BiosSlot from '@/components/platform/BiosSlot.vue'
import GamesTab from '@/components/platform/GamesTab.vue'
import ResourceTab from '@/components/platform/ResourceTab.vue'
import Breadcrumbs, { type Crumb } from '@/components/layout/Breadcrumbs.vue'
import { Gamepad2, Layers } from 'lucide-vue-next'

type TabKey = 'games' | 'video' | 'bezels' | 'bios'

const props = defineProps<{ tag: string; tab?: string }>()
const router = useRouter()
const { t } = useI18n()

const displayName = computed(() => platformName(props.tag))

const hasVideo = computed(() => props.tag === VIDEO_HOME)

const activeTab = computed<TabKey>(() => {
  if (props.tab === 'bezels' || props.tab === 'bios') return props.tab
  return props.tab === 'video' && hasVideo.value ? 'video' : 'games'
})

const gamesTag = computed(() => (activeTab.value === 'video' ? VIDEO_TAG : props.tag))

const crumbs = computed<Crumb[]>(() => [
  { label: t('platform.crumbPlatforms'), to: { name: 'dashboard' } },
  { label: displayName.value },
])

const tabs = computed(() => [
  { key: 'games' as TabKey, label: t('platform.tabGames') },
  ...(hasVideo.value ? [{ key: 'video' as TabKey, label: t('platform.tabVideo') }] : []),
  { key: 'bezels' as TabKey, label: t('platform.tabBezels') },
  { key: 'bios' as TabKey, label: t('platform.tabBios') },
])

function selectTab(tab: TabKey) {
  if (tab === 'games') router.replace({ name: 'platform', params: { tag: props.tag } })
  else if (tab === 'video') router.replace({ name: 'platform-video', params: { tag: props.tag } })
  else if (tab === 'bios') router.replace({ name: 'platform-bios', params: { tag: props.tag } })
  else router.replace({ name: 'platform-bezels', params: { tag: props.tag } })
}

const gamesTabRef = ref<InstanceType<typeof GamesTab> | null>(null)
const bezelsTabRef = ref<InstanceType<typeof ResourceTab> | null>(null)

const actionItems = computed<DropdownItem[]>(() => {
  if (activeTab.value === 'bios') return []
  if (activeTab.value !== 'bezels') {
    return [
      { label: t('platform.uploadRoms'), icon: Gamepad2, onSelect: () => gamesTabRef.value?.triggerRomUpload() },
    ]
  }
  return [
    { label: t('platform.uploadBezel'), icon: Layers, onSelect: () => bezelsTabRef.value?.triggerUpload() },
  ]
})
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6 space-y-6">
    <div class="flex flex-wrap items-center gap-3">
      <Breadcrumbs :items="crumbs" />
      <div class="sm:ml-auto">
        <Dropdown v-if="actionItems.length" :items="actionItems" />
      </div>
    </div>

    <div class="flex items-center gap-1 border-b border-border">
      <button
        v-for="t in tabs"
        :key="t.key"
        class="slot-label first:pl-0 px-3 py-1.5 text-xs transition-colors border-b-2 -mb-px whitespace-nowrap"
        :class="activeTab === t.key ? 'border-accent text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'"
        :aria-current="activeTab === t.key ? 'page' : undefined"
        @click="selectTab(t.key)"
      >
        {{ t.label }}
      </button>
    </div>

    <div>
      <BiosSlot v-if="activeTab === 'bios'" :key="props.tag" :tag="props.tag" />
      <GamesTab v-else-if="activeTab !== 'bezels'" :key="gamesTag" ref="gamesTabRef" :tag="gamesTag" />
      <ResourceTab
        v-else
        key="bezels"
        ref="bezelsTabRef"
        :tag="props.tag"
        resource="overlays"
        display="images"
      />
    </div>
  </div>
</template>
