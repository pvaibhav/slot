<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { uploadGameArt, fetchGameArt, deleteGameArt, deleteGame, getGame, renameGame, StaleListError, type GameDetail } from '@/api/client'
import { platformName, platformRoute } from '@/api/platforms'
import { cartAspect } from '@/lib/coverMode'
import { useCartImage } from '@/composables/useCartImage'
import { confirm } from '@/lib/confirm'
import { toast } from '@/lib/toast'
import Button from '@/components/ui/Button.vue'
import Modal from '@/components/ui/Modal.vue'
import Dropdown from '@/components/ui/Dropdown.vue'
import type { DropdownItem } from '@/components/ui/Dropdown.vue'
import RenameDialog from '@/components/file/RenameDialog.vue'
import Breadcrumbs, { type Crumb } from '@/components/layout/Breadcrumbs.vue'
import { ImagePlus, ImageOff, Trash2, Pencil, CloudDownload } from 'lucide-vue-next'
import RomTab from '@/components/game/RomTab.vue'
import SavesTab from '@/components/game/SavesTab.vue'
import StatesTab from '@/components/game/StatesTab.vue'
import GuidesTab from '@/components/game/GuidesTab.vue'

type TabKey = 'rom' | 'saves' | 'states' | 'guides'
const TAB_KEYS: readonly TabKey[] = ['rom', 'saves', 'states', 'guides']

const props = defineProps<{ tag: string; id: string; tab?: string }>()
const romId = computed(() => Number(props.id))
const router = useRouter()
const route = useRoute()
const { t } = useI18n()

const game = ref<GameDetail | null>(null)

function backRoute() {
  const query = route.query.page ? { page: route.query.page } : {}
  return platformRoute(props.tag, query)
}

const crumbs = computed<Crumb[]>(() => {
  const query = route.query.page ? { page: route.query.page } : {}
  return [
    { label: t('platform.crumbPlatforms'), to: { name: 'dashboard' } },
    { label: platformName(props.tag), to: platformRoute(props.tag, query) },
    { label: game.value?.displayName ?? '' },
  ]
})
const loading = ref(true)
const error = ref<string | null>(null)
const hero = ref<HTMLElement | null>(null)
const cartSrc = useCartImage(() => game.value?.cartUrl, hero)
const showDelete = ref(false)
const purge = ref(false)
const deleting = ref(false)
const showRename = ref(false)
const fetching = ref(false)

const activeTab = computed<TabKey>(() =>
  TAB_KEYS.includes(props.tab as TabKey) ? (props.tab as TabKey) : 'rom',
)

function selectTab(tab: TabKey) {
  router.replace(`/platform/${encodeURIComponent(props.tag)}/game/${props.id}/${tab}`)
}

watch(() => props.tab, t => {
  if (t && !TAB_KEYS.includes(t as TabKey)) selectTab('rom')
}, { immediate: true })

const tabs = computed(() => {
  const g = game.value
  return [
    { key: 'rom' as TabKey, label: t('game.tabGameFiles'), count: null as number | null },
    { key: 'saves' as TabKey, label: t('game.tabSaves'), count: g?.savesCount ?? 0 },
    { key: 'states' as TabKey, label: t('game.tabSaveStates'), count: g?.statesCount ?? 0 },
    { key: 'guides' as TabKey, label: t('game.tabGuides'), count: g?.guidesCount ?? 0 },
  ]
})

async function loadGame() {
  loading.value = true
  error.value = null
  try {
    game.value = await getGame(props.tag, romId.value)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('game.loadFailed')
  } finally {
    loading.value = false
  }
}

async function reloadRow() {
  game.value = await getGame(props.tag, romId.value)
}

const labelInput = ref<HTMLInputElement | null>(null)

async function onLabelPicked(event: Event) {
  const el = event.target as HTMLInputElement
  const file = el.files?.[0]
  el.value = ''
  if (!file) return
  try {
    await uploadGameArt(props.tag, romId.value, file)
    await reloadRow()
  } catch {
    error.value = t('game.artUploadFailed')
  }
}

async function onFetchArt() {
  if (fetching.value) return
  if (game.value?.hasArt && !await confirm({ title: t('game.fetchReplaceTitle'), confirmLabel: t('game.fetchReplace') })) return
  fetching.value = true
  toast(t('game.fetchingLabel'), true)
  try {
    const found = await fetchGameArt(props.tag, romId.value)
    await reloadRow()
    toast(found ? t('game.fetchedLabel') : t('game.fetchNoLabel'))
  } catch {
    toast(t('game.fetchFailed'))
  } finally {
    fetching.value = false
  }
}

async function onDeleteArt() {
  if (!await confirm({ title: t('game.deleteBoxArtTitle'), confirmLabel: t('common.delete'), destructive: true })) return
  try {
    await deleteGameArt(props.tag, romId.value)
    await reloadRow()
  } catch {
    error.value = t('game.artDeleteFailed')
  }
}

async function confirmDelete() {
  deleting.value = true
  try {
    await deleteGame(props.tag, romId.value, game.value!.rom, purge.value)
    router.push(backRoute())
  } catch (e) {
    error.value = e instanceof StaleListError ? t('game.staleList') : t('game.deleteGameFailed')
    deleting.value = false
    showDelete.value = false
  }
}

const actionItems = computed<DropdownItem[]>(() => {
  const items: DropdownItem[] = [
    { label: t('common.rename'), icon: Pencil, onSelect: () => { showRename.value = true } },
  ]
  items.push(
    { label: t('game.fetchLabel'), icon: CloudDownload, onSelect: onFetchArt },
    { label: t('game.customLabel'), icon: ImagePlus, onSelect: () => labelInput.value?.click() },
  )
  if (game.value?.hasArt) {
    items.push({ label: t('game.removeArt'), icon: ImageOff, danger: true, onSelect: onDeleteArt })
  }
  items.push({ label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => { purge.value = false; showDelete.value = true } })
  return items
})

async function onRenameGame(newName: string) {
  try {
    await renameGame(props.tag, romId.value, newName)
    await loadGame()
  } catch {
    error.value = t('game.renameGameFailed')
  } finally {
    showRename.value = false
  }
}

onMounted(loadGame)
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6">
    <input ref="labelInput" type="file" class="hidden" @change="onLabelPicked" />
    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('common.loading') }}</p>
    <div v-else-if="error" class="text-base py-8 text-center space-y-2">
      <p class="text-destructive">{{ error }}</p>
      <Button variant="outline" size="sm" @click="loadGame">{{ $t('common.retry') }}</Button>
    </div>
    <template v-else-if="game">
      <div class="flex items-center justify-between gap-3 mb-4">
        <Breadcrumbs :items="crumbs" />
        <Dropdown class="shrink-0" :items="actionItems" />
      </div>

      <div ref="hero" class="relative rounded-xl overflow-hidden bg-card">
        <template v-if="cartSrc">
          <img
            :src="cartSrc"
            aria-hidden="true"
            class="absolute inset-0 h-full w-full object-cover"
            style="filter: blur(28px) brightness(0.5); transform: scale(1.25)"
          />
          <div class="absolute inset-0 bg-black/40"></div>
        </template>
        <div class="relative p-5">
          <div class="flex flex-col sm:flex-row sm:items-end gap-5">
            <div class="shrink-0 h-32 max-w-full" :style="{ aspectRatio: String(cartAspect(props.tag)) }">
              <img
                v-if="cartSrc"
                :src="cartSrc"
                :alt="game.displayName"
                loading="lazy"
                decoding="async"
                class="h-full w-full object-contain drop-shadow-xl"
              />
            </div>
            <div class="min-w-0 pb-1">
              <h1 class="text-2xl sm:text-3xl font-extrabold tracking-tight text-white break-words">{{ game.displayName }}</h1>
              <div class="text-sm text-white/75 mt-1.5">{{ game.platformDisplayName }}</div>
            </div>
          </div>
        </div>
      </div>

      <div class="flex items-center gap-1 border-b border-border mt-6">
        <button
          v-for="t in tabs"
          :key="t.key"
          class="slot-label hidden sm:inline-flex items-center first:pl-0 px-3 py-1.5 text-xs transition-colors border-b-2 -mb-px whitespace-nowrap"
          :class="activeTab === t.key ? 'border-accent text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'"
          :aria-current="activeTab === t.key ? 'page' : undefined"
          @click="selectTab(t.key)"
        >
          {{ t.label }}<span v-if="t.count !== null"> ({{ t.count }})</span>
        </button>
        <select
          class="sm:hidden flex-1 my-2 rounded-lg border border-border bg-card px-3 py-2 text-base font-medium"
          :value="activeTab"
          @change="selectTab(($event.target as HTMLSelectElement).value as TabKey)"
        >
          <option v-for="t in tabs" :key="t.key" :value="t.key">
            {{ t.label }}{{ t.count !== null ? ` (${t.count})` : '' }}
          </option>
        </select>
        <div id="game-tab-actions" class="ml-auto flex items-center gap-2 shrink-0"></div>
      </div>

      <div class="mt-6">
        <keep-alive>
          <RomTab v-if="activeTab === 'rom'" :tag="tag" :id="romId" />
          <SavesTab v-else-if="activeTab === 'saves'" :tag="tag" :id="romId" @changed="reloadRow" />
          <StatesTab v-else-if="activeTab === 'states'" :tag="tag" :id="romId" :rom-name="game.rom" />
          <GuidesTab v-else-if="activeTab === 'guides'" :tag="tag" :id="romId" />
                  </keep-alive>
      </div>

      <RenameDialog
        v-if="showRename && game"
        :current-name="game.displayName"
        @close="showRename = false"
        @rename="onRenameGame"
      />

      <Modal v-if="showDelete" :title="$t('game.deleteGameTitle')" @close="showDelete = false">
        <p class="text-sm text-foreground/70 mt-1">
          {{ $t('game.deleteGameBody', { name: game.displayName }) }}
        </p>
        <label class="flex items-center gap-2.5 text-sm text-foreground/85 cursor-pointer">
          <input type="checkbox" v-model="purge" class="h-4 w-4 accent-destructive" />
          {{ $t('game.deleteGamePurgeLabel') }}
        </label>
        <template #footer>
          <Button variant="ghost" :disabled="deleting" @click="showDelete = false">{{ $t('common.cancel') }}</Button>
          <Button variant="destructive" :disabled="deleting" @click="confirmDelete">
            {{ deleting ? $t('game.deleting') : $t('common.delete') }}
          </Button>
        </template>
      </Modal>
    </template>
  </div>
</template>
