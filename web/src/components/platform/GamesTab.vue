<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { getGames, uploadFiles, renameGame, deleteGame, StaleListError, type GameRow } from '@/api/client'
import Button from '@/components/ui/Button.vue'
import Input from '@/components/ui/Input.vue'
import Progress from '@/components/ui/Progress.vue'
import { shownTitles } from '@/lib/names'
import GameCover from '@/components/game/GameCover.vue'
import { MIN_PAGE_SIZE, pageSizeFor } from '@/lib/coverMode'
import GameTable from '@/components/game/GameTable.vue'
import Modal from '@/components/ui/Modal.vue'
import RenameDialog from '@/components/file/RenameDialog.vue'
import { Check, CheckSquare, LayoutGrid, Table as TableIcon, Search, Upload } from 'lucide-vue-next'
import { useCardSelection } from '@/composables/useCardSelection'

const MIN_COL_PX = 220

const props = defineProps<{ tag: string }>()
const router = useRouter()
const route = useRoute()
const { t } = useI18n()

const games = ref<GameRow[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const search = ref('')

const viewMode = ref<'cards' | 'table'>(
  localStorage.getItem('slot_view_mode') === 'table' ? 'table' : 'cards',
)
watch(viewMode, v => localStorage.setItem('slot_view_mode', v))

const gridCols = ref(1)

const titles = computed(() => shownTitles(games.value.map(g => g.displayName)))

const filtered = computed(() => {
  const q = search.value.toLowerCase().trim()
  if (!q) return games.value
  return games.value.filter(g => g.displayName.toLowerCase().includes(q))
})

const gridRef = ref<HTMLElement | null>(null)
const pageSize = ref(MIN_PAGE_SIZE)

function recomputeCols() {
  const el = gridRef.value
  if (!el) return
  const gap = parseFloat(getComputedStyle(el).columnGap) || 0
  const width = el.clientWidth
  if (width <= 0) return
  gridCols.value = Math.max(1, Math.floor((width + gap) / (MIN_COL_PX + gap)))
}

function recomputePageSize() {
  recomputeCols()
  pageSize.value = viewMode.value === 'table' ? MIN_PAGE_SIZE : pageSizeFor(gridCols.value)
}

let resizeObserver: ResizeObserver | null = null
onMounted(() => {
  nextTick(recomputePageSize)
  resizeObserver = new ResizeObserver(() => recomputePageSize())
  window.addEventListener('resize', recomputePageSize)
})
onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  window.removeEventListener('resize', recomputePageSize)
})
watch(gridRef, el => {
  resizeObserver?.disconnect()
  if (el) resizeObserver?.observe(el)
  nextTick(recomputePageSize)
})

const page = ref(Math.max(1, Number(route.query.page) || 1))

watch(page, p => {
  const query = { ...route.query }
  if (p > 1) query.page = String(p)
  else delete query.page
  router.replace({ query })
})
const sortKey = ref<string>('title')
const sortDir = ref<1 | -1>(1)

function setSort(key: string) {
  if (sortKey.value === key) {
    sortDir.value = (sortDir.value * -1) as 1 | -1
  } else {
    sortKey.value = key
    sortDir.value = 1
  }
}

const ordered = computed(() => {
  if (viewMode.value !== 'table') return filtered.value
  const arr = [...filtered.value]
  const d = sortDir.value
  arr.sort((a, b) => {
    let r: number
    switch (sortKey.value) {
      case 'saves': r = a.savesCount - b.savesCount; break
      case 'states': r = a.statesCount - b.statesCount; break
      case 'guides': r = a.guidesCount - b.guidesCount; break
      default: r = a.displayName.localeCompare(b.displayName)
    }
    return r * d
  })
  return arr
})

const pageCount = computed(() => Math.max(1, Math.ceil(ordered.value.length / pageSize.value)))
const paged = computed(() => {
  const start = (page.value - 1) * pageSize.value
  return ordered.value.slice(start, start + pageSize.value)
})

watch([viewMode, () => filtered.value.length], () => nextTick(recomputePageSize))
watch([search, viewMode, sortKey, sortDir], () => { page.value = 1 })
watch(pageCount, pc => { if (page.value > pc) page.value = pc })

async function load() {
  loading.value = true
  error.value = null
  try {
    const res = await getGames(props.tag)
    games.value = res.games
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('platform.loadFailed')
  } finally {
    loading.value = false
  }
}

async function reload() {
  try {
    const res = await getGames(props.tag)
    games.value = res.games
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('platform.loadFailed')
  }
}

const fileInput = ref<HTMLInputElement>()
const dragOver = ref(false)

const {
  selectMode,
  selectedGameIds,
  selectedCount,
  toggleSelectMode,
  cancelPress,
  startPressGame,
  handleGameClick,
} = useCardSelection({ openGame })

const renamePromptOpen = ref(false)
const deletePromptOpen = ref(false)
const purge = ref(false)

const renameTarget = computed(() => {
  if (selectedGameIds.size !== 1) return null
  const id = [...selectedGameIds][0]!
  const game = games.value.find(g => g.id === id)
  return game ? { name: game.displayName, gameId: id } : null
})

function endSelection() {
  if (selectMode.value) toggleSelectMode()
}

async function onRename(newName: string) {
  const target = renameTarget.value
  if (!target) return
  try {
    await renameGame(props.tag, target.gameId, newName)
    await reload()
  } catch {
    error.value = t('platform.renameFailed')
  } finally {
    renamePromptOpen.value = false
    endSelection()
  }
}

async function onDelete() {
  const picked = games.value
    .filter(g => selectedGameIds.has(g.id))
    .sort((a, b) => b.id - a.id)
  try {
    for (const game of picked) {
      await deleteGame(props.tag, game.id, game.rom, purge.value)
    }
    await reload()
  } catch (e) {
    if (e instanceof StaleListError) {
      await reload()
      error.value = t('platform.staleList')
    } else {
      await reload()
      error.value = t('platform.deleteFailed')
    }
  } finally {
    deletePromptOpen.value = false
    purge.value = false
    endSelection()
  }
}

const uploading = ref(false)
const uploadProgress = ref(0)
const uploadName = ref('')
const uploadCurrent = ref(0)
const uploadTotal = ref(0)

async function doUpload(files: File[]) {
  if (!files.length || uploading.value) return
  uploading.value = true
  error.value = null
  uploadTotal.value = files.length
  try {
    for (let i = 0; i < files.length; i++) {
      const file = files[i]!
      uploadCurrent.value = i + 1
      uploadName.value = file.name
      uploadProgress.value = 0
      const { promise } = uploadFiles('roms', [props.tag], [file], pct => { uploadProgress.value = pct })
      await promise
    }
    await reload()
  } catch {
    error.value = t('platform.romUploadFailed')
  } finally {
    uploading.value = false
  }
}

let dragDepth = 0
function onWindowDragEnter(e: DragEvent) {
  if (!e.dataTransfer?.types.includes('Files')) return
  e.preventDefault()
  dragDepth++
  dragOver.value = true
}
function onWindowDragOver(e: DragEvent) {
  e.preventDefault()
}
function onWindowDragLeave(e: DragEvent) {
  e.preventDefault()
  dragDepth = Math.max(0, dragDepth - 1)
  if (dragDepth === 0) dragOver.value = false
}
function onWindowDrop(e: DragEvent) {
  e.preventDefault()
  dragDepth = 0
  dragOver.value = false
  doUpload(Array.from(e.dataTransfer?.files ?? []))
}

function handleFiles(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  doUpload(files)
}

function openGame(id: number) {
  const query = page.value > 1 ? { page: String(page.value) } : undefined
  router.push({ path: `/platform/${encodeURIComponent(props.tag)}/game/${id}`, query })
}

function triggerRomUpload() {
  fileInput.value?.click()
}

defineExpose({ triggerRomUpload, reload })

onMounted(() => {
  load()
  window.addEventListener('dragenter', onWindowDragEnter)
  window.addEventListener('dragover', onWindowDragOver)
  window.addEventListener('dragleave', onWindowDragLeave)
  window.addEventListener('drop', onWindowDrop)
})
onBeforeUnmount(() => {
  window.removeEventListener('dragenter', onWindowDragEnter)
  window.removeEventListener('dragover', onWindowDragOver)
  window.removeEventListener('dragleave', onWindowDragLeave)
  window.removeEventListener('drop', onWindowDrop)
})
</script>

<template>
  <div class="space-y-6">
    <input ref="fileInput" type="file" multiple class="hidden" @change="handleFiles" />
    <RenameDialog v-if="renamePromptOpen && renameTarget" :current-name="renameTarget.name" @close="renamePromptOpen = false" @rename="onRename" />
    <Modal v-if="deletePromptOpen" :title="$t('platform.deleteTitle', { n: selectedCount }, selectedCount)" @close="deletePromptOpen = false">
      <p class="text-sm text-foreground/80">{{ $t('platform.deleteBody') }}</p>
      <label class="flex items-center gap-2.5 text-sm text-foreground/85 cursor-pointer">
        <input v-model="purge" type="checkbox" class="h-4 w-4 accent-destructive" />
        {{ $t('game.deleteGamePurgeLabel') }}
      </label>
      <template #footer>
        <Button variant="ghost" @click="deletePromptOpen = false">{{ $t('common.cancel') }}</Button>
        <Button variant="destructive" @click="onDelete">{{ $t('common.delete') }}</Button>
      </template>
    </Modal>

    <div v-if="uploading" class="rounded-lg border border-border bg-card p-3 space-y-2">
      <div class="flex items-center justify-between text-sm">
        <span class="text-foreground font-medium truncate">
          {{ uploadName }}<span v-if="uploadTotal > 1" class="text-foreground/60 font-normal"> {{ $t('platform.uploadCount', { current: uploadCurrent, total: uploadTotal }) }}</span>
        </span>
        <span class="font-mono text-foreground/60 ml-2 shrink-0">{{ uploadProgress }}%</span>
      </div>
      <Progress :value="uploadProgress" />
    </div>

    <div
      v-if="selectMode && selectedCount > 0"
      class="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-3 py-2"
    >
      <span class="text-sm font-medium text-foreground">{{ $t('platform.selectedCount', { n: selectedCount }) }}</span>
      <div class="flex items-center gap-2 sm:ml-auto flex-wrap">
        <button
          class="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!renameTarget"
          @click="renamePromptOpen = true"
        >{{ $t('common.rename') }}</button>
        <button
          class="rounded-md border border-destructive/60 px-3 py-1.5 text-xs font-medium text-destructive transition-colors hover:bg-destructive/10"
          @click="purge = false; deletePromptOpen = true"
        >{{ $t('common.delete') }}</button>
        <button
          class="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground/70 transition-colors hover:text-foreground hover:bg-muted"
          @click="toggleSelectMode"
        >{{ $t('platform.done') }}</button>
      </div>
    </div>

    <div class="flex items-center justify-between gap-3">
      <div class="relative w-64">
        <Search class="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-foreground/70 pointer-events-none" />
        <Input v-model="search" :placeholder="$t('platform.searchPlaceholder')" class="!pl-9 !h-10 !text-base !rounded-lg" />
      </div>
      <div class="flex items-center gap-2">
        <button
          class="flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium transition-colors"
          :class="selectMode ? 'border-accent bg-accent/10 text-foreground' : 'border-border text-foreground/70 hover:text-foreground'"
          :aria-pressed="selectMode"
          :title="$t('platform.selectItems')"
          @click="toggleSelectMode"
        >
          <CheckSquare class="h-4 w-4" />
          {{ $t('platform.select') }}
        </button>
        <div class="flex rounded-lg border border-border overflow-hidden">
          <button
            class="p-2 transition-colors"
            :class="viewMode === 'cards' ? 'bg-accent/10 text-accent' : 'text-foreground/70 hover:text-foreground'"
            :aria-pressed="viewMode === 'cards'"
            :title="$t('platform.cardView')"
            @click="viewMode = 'cards'"
          >
            <LayoutGrid class="h-4 w-4" />
          </button>
          <button
            class="p-2 transition-colors border-l border-border"
            :class="viewMode === 'table' ? 'bg-accent/10 text-accent' : 'text-foreground/70 hover:text-foreground'"
            :aria-pressed="viewMode === 'table'"
            :title="$t('platform.tableView')"
            @click="viewMode = 'table'"
          >
            <TableIcon class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>

    <p v-if="error && !loading" class="text-base text-center text-destructive">{{ error }}</p>
    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('platform.loadingGames') }}</p>
    <div v-else-if="error && !games.length" class="text-base py-4 text-center">
      <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
    </div>
    <p v-else-if="!filtered.length" class="text-base text-foreground/75 py-8 text-center">
      {{ search ? $t('platform.noGamesMatch', { query: search }) : $t('platform.noGamesYet') }}
    </p>
    <section v-else-if="viewMode === 'cards'">
      <div
        ref="gridRef"
        class="grid gap-3 sm:gap-4"
        style="grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));"
      >
        <button
          v-for="game in paged"
          :key="game.id"
          class="relative text-left transition-transform select-none"
          :class="!selectMode ? 'hover:-translate-y-1' : selectedGameIds.has(game.id) ? 'ring-2 ring-accent rounded-md' : ''"
          @click="handleGameClick(game.id, $event)"
          @pointerdown="startPressGame(game.id, $event)"
          @pointerup="cancelPress"
          @pointerleave="cancelPress"
          @pointercancel="cancelPress"
          @contextmenu.prevent
        >
          <span
            v-if="selectMode"
            class="absolute top-1.5 left-1.5 z-10 inline-flex h-5 w-5 items-center justify-center rounded border-2"
            :class="selectedGameIds.has(game.id) ? 'bg-accent border-accent' : 'border-white/60 bg-black/60'"
          >
            <Check v-if="selectedGameIds.has(game.id)" class="h-3 w-3 text-accent-foreground" />
          </span>
          <GameCover :tag="props.tag" :game="game" :title="titles.get(game.displayName)" />
        </button>
      </div>
    </section>
    <GameTable
      v-else
      :games="paged"
      :titles="titles"
      :sort-key="sortKey"
      :sort-dir="sortDir"
      :select-mode="selectMode"
      :selected-game-ids="selectedGameIds"
      @sort="setSort"
      @click-game="handleGameClick"
      @press-game="startPressGame"
      @cancel-press="cancelPress"
    />

    <div v-if="!loading && pageCount > 1" class="flex items-center justify-center gap-3 pt-1">
      <Button variant="outline" size="sm" :disabled="page <= 1" @click="page--">{{ $t('platform.prev') }}</Button>
      <span class="text-sm text-foreground/70">{{ $t('platform.pageOf', { page, total: pageCount }) }}</span>
      <Button variant="outline" size="sm" :disabled="page >= pageCount" @click="page++">{{ $t('platform.next') }}</Button>
    </div>

    <div
      v-if="dragOver"
      class="fixed inset-0 z-50 border-4 border-dashed border-accent bg-background/90 flex flex-col items-center justify-center gap-3 pointer-events-none"
    >
      <Upload class="h-12 w-12 text-accent" />
      <p class="text-xl font-semibold text-foreground">{{ $t('platform.dropRomsToUpload') }}</p>
    </div>
  </div>
</template>
