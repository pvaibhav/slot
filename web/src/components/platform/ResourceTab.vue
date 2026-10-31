<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { listFiles, deleteFile, uploadFiles, resourceFileBlob, type FileEntry } from '@/api/client'
import { formatSize, isPreviewable } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import Progress from '@/components/ui/Progress.vue'
import { confirm } from '@/lib/confirm'
import { Check, File as FileIcon, LayoutGrid, Table as TableIcon, Trash2, Upload, Eye } from 'lucide-vue-next'

const props = defineProps<{ tag: string; resource: 'overlays'; display?: 'list' | 'images'; subPath?: string[]; emptyLabel?: string }>()

const router = useRouter()
const { t } = useI18n()
const resourceLabel = computed(() => t('platform.tabBezels'))

function openView(name: string) {
  router.push({
    name: 'file-view',
    query: {
      resource: props.resource,
      tag: props.tag,
      ...(props.subPath?.length ? { path: props.subPath.join('/') } : {}),
      name,
      rlabel: `/${resourceLabel.value}/${props.tag}`,
    },
  })
}

const entries = ref<FileEntry[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const deleting = ref(false)

const fileInput = ref<HTMLInputElement>()
const uploading = ref(false)
const uploadProgress = ref(0)
const uploadName = ref('')
const uploadCurrent = ref(0)
const uploadTotal = ref(0)

const selected = reactive(new Set<string>())
const thumbnails = reactive(new Map<string, string>())

const viewMode = ref<'list' | 'images'>(props.display ?? 'list')
watch(() => props.display, d => { if (d) viewMode.value = d })

const emptyMessage = computed(() =>
  props.emptyLabel ?? t('platform.noBezelsYet'),
)

function clearThumbnails() {
  for (const url of thumbnails.values()) URL.revokeObjectURL(url)
  thumbnails.clear()
}

async function loadThumbnails() {
  for (const entry of entries.value) {
    if (thumbnails.has(entry.name)) continue
    const url = await resourceFileBlob(props.resource, props.tag, entry.name)
    if (url) thumbnails.set(entry.name, url)
  }
}

async function load() {
  loading.value = true
  error.value = null
  clearThumbnails()
  selected.clear()
  try {
    const res = await listFiles(props.resource, props.tag, ...(props.subPath ?? []))
    entries.value = (res.entries ?? []).filter(e => e.type === 'file')
    if (props.display === 'images' || viewMode.value === 'images') await loadThumbnails()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('platform.loadFilesFailed')
    entries.value = []
  } finally {
    loading.value = false
  }
}

watch(viewMode, async v => {
  if (v === 'images') await loadThumbnails()
})

async function deleteSelected() {
  if (!selected.size) return
  const title = t('browse.deleteSelectedConfirm', selected.size)
  if (!await confirm({ title, confirmLabel: t('common.delete'), destructive: true })) return
  deleting.value = true
  try {
    for (const name of [...selected]) await deleteFile(props.resource, props.tag, ...(props.subPath ?? []), name)
    await load()
  } catch {
    error.value = t('platform.deleteFailed')
  } finally {
    deleting.value = false
  }
}

async function deleteOne(name: string) {
  const title = t('browse.deleteFileConfirm', { name })
  if (!await confirm({ title, confirmLabel: t('common.delete'), destructive: true })) return
  deleting.value = true
  try {
    await deleteFile(props.resource, props.tag, ...(props.subPath ?? []), name)
    await load()
  } catch {
    error.value = t('platform.deleteFailed')
  } finally {
    deleting.value = false
  }
}

function toggleSelect(name: string) {
  if (selected.has(name)) selected.delete(name)
  else selected.add(name)
}

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
      const { promise } = uploadFiles(props.resource, [props.tag, ...(props.subPath ?? [])], [file], pct => { uploadProgress.value = pct })
      await promise
    }
    await load()
  } catch {
    error.value = t('platform.uploadFailed')
  } finally {
    uploading.value = false
  }
}

function handleFiles(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  doUpload(files)
}

function triggerUpload() {
  fileInput.value?.click()
}

defineExpose({ triggerUpload })

const dragOver = ref(false)
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

watch(() => [props.tag, props.resource, (props.subPath ?? []).join('/')], load)
onMounted(() => {
  load()
  window.addEventListener('dragenter', onWindowDragEnter)
  window.addEventListener('dragover', onWindowDragOver)
  window.addEventListener('dragleave', onWindowDragLeave)
  window.addEventListener('drop', onWindowDrop)
})
onBeforeUnmount(() => {
  clearThumbnails()
  window.removeEventListener('dragenter', onWindowDragEnter)
  window.removeEventListener('dragover', onWindowDragOver)
  window.removeEventListener('dragleave', onWindowDragLeave)
  window.removeEventListener('drop', onWindowDrop)
})
</script>

<template>
  <div class="space-y-4">
    <input ref="fileInput" type="file" multiple class="hidden" @change="handleFiles" />

    <div v-if="uploading" class="rounded-lg border border-border bg-card p-3 space-y-2">
      <div class="flex items-center justify-between text-sm">
        <span class="text-foreground font-medium truncate">
          {{ uploadName }}<span v-if="uploadTotal > 1" class="text-foreground/60 font-normal"> {{ $t('platform.uploadCount', { current: uploadCurrent, total: uploadTotal }) }}</span>
        </span>
        <span class="font-mono text-foreground/60 ml-2 shrink-0">{{ uploadProgress }}%</span>
      </div>
      <Progress :value="uploadProgress" />
    </div>

    <div class="flex items-center justify-end gap-2">
      <div class="flex rounded-lg border border-border overflow-hidden">
        <button
          class="p-2 transition-colors"
          :class="viewMode === 'images' ? 'bg-accent/10 text-accent' : 'text-foreground/70 hover:text-foreground'"
          :aria-pressed="viewMode === 'images'"
          :title="$t('platform.imageView')"
          @click="viewMode = 'images'"
        >
          <LayoutGrid class="h-4 w-4" />
        </button>
        <button
          class="p-2 transition-colors border-l border-border"
          :class="viewMode === 'list' ? 'bg-accent/10 text-accent' : 'text-foreground/70 hover:text-foreground'"
          :aria-pressed="viewMode === 'list'"
          :title="$t('platform.listView')"
          @click="viewMode = 'list'"
        >
          <TableIcon class="h-4 w-4" />
        </button>
      </div>
    </div>

    <div
      v-if="selected.size > 0"
      class="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-3 py-2"
    >
      <span class="text-sm font-medium text-foreground">{{ $t('platform.selectedCount', { n: selected.size }) }}</span>
      <div class="flex items-center gap-2 sm:ml-auto flex-wrap">
        <button
          class="rounded-md border border-destructive/60 px-3 py-1.5 text-xs font-medium text-destructive transition-colors hover:bg-destructive/10 disabled:opacity-50"
          :disabled="deleting"
          @click="deleteSelected"
        >{{ $t('common.delete') }}</button>
        <button
          class="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground/70 transition-colors hover:text-foreground hover:bg-muted"
          @click="selected.clear()"
        >{{ $t('platform.clear') }}</button>
      </div>
    </div>

    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('common.loading') }}</p>
    <div v-else-if="error" class="text-base py-8 text-center space-y-2">
      <p class="text-destructive">{{ error }}</p>
      <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
    </div>
    <p v-else-if="!entries.length" class="text-base text-foreground/75 py-8 text-center">
      {{ emptyMessage }}
    </p>

    <div
      v-else-if="viewMode === 'images'"
      class="grid grid-cols-2 sm:grid-cols-3 gap-4"
    >
      <div
        v-for="entry in entries"
        :key="entry.name"
        class="group relative cursor-pointer"
        :class="selected.has(entry.name) ? 'ring-2 ring-accent rounded-lg' : ''"
        @click="toggleSelect(entry.name)"
      >
        <div
          class="relative bg-card border border-border rounded-lg overflow-hidden flex items-center justify-center"
          style="aspect-ratio: 10 / 9"
        >
          <img
            v-if="thumbnails.get(entry.name)"
            :src="thumbnails.get(entry.name)"
            :alt="entry.name"
            class="h-full w-full object-contain"
          />
          <FileIcon v-else class="h-10 w-10 text-muted-foreground" />
          <div
            v-if="selected.has(entry.name)"
            class="absolute top-2 left-2 inline-flex h-5 w-5 items-center justify-center rounded bg-accent"
          >
            <Check class="h-3 w-3 text-accent-foreground" />
          </div>
          <div class="absolute top-2 right-2 flex gap-1.5">
            <button
              v-if="isPreviewable(entry.name)"
              class="inline-flex h-7 w-7 items-center justify-center rounded bg-black/60 text-white transition-colors hover:bg-accent hover:text-accent-foreground"
              :title="$t('common.view')"
              @click.stop="openView(entry.name)"
            >
              <Eye class="h-3.5 w-3.5" />
            </button>
            <button
              class="inline-flex h-7 w-7 items-center justify-center rounded bg-black/60 text-white transition-colors hover:bg-destructive disabled:opacity-50"
              :disabled="deleting"
              :title="$t('common.delete')"
              @click.stop="deleteOne(entry.name)"
            >
              <Trash2 class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
        <div class="mt-1.5 text-xs text-foreground/80 truncate text-center" :title="entry.name">{{ entry.name }}</div>
      </div>
    </div>

    <div v-else class="rounded-xl border border-border overflow-hidden">
      <div
        v-for="(entry, idx) in entries"
        :key="entry.name"
        class="group flex items-center gap-3 px-4 py-3 hover:bg-muted/50 cursor-pointer"
        :class="[idx > 0 ? 'border-t border-border' : '', selected.has(entry.name) ? 'bg-accent/5' : '']"
        @click="toggleSelect(entry.name)"
      >
        <span
          class="inline-flex h-4 w-4 items-center justify-center rounded border-2 border-foreground/40 shrink-0"
          :class="selected.has(entry.name) ? 'bg-accent border-accent' : ''"
        >
          <Check v-if="selected.has(entry.name)" class="h-2.5 w-2.5 text-accent-foreground" />
        </span>
        <FileIcon class="h-4 w-4 text-muted-foreground shrink-0" />
        <span class="flex-1 truncate text-sm">{{ entry.name }}</span>
        <span class="text-xs text-muted-foreground tabular-nums">{{ formatSize(entry.size) }}</span>
        <button
          v-if="isPreviewable(entry.name)"
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-accent hover:bg-accent/10 transition-colors"
          :title="$t('common.view')"
          @click.stop="openView(entry.name)"
        >
          <Eye class="h-3.5 w-3.5" />
        </button>
        <button
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-50"
          :disabled="deleting"
          :title="$t('common.delete')"
          @click.stop="deleteOne(entry.name)"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </button>
      </div>
    </div>

    <div
      v-if="dragOver"
      class="fixed inset-0 z-50 border-4 border-dashed border-accent bg-background/90 flex flex-col items-center justify-center gap-3 pointer-events-none"
    >
      <Upload class="h-12 w-12 text-accent" />
      <p class="text-xl font-semibold text-foreground">{{ $t('platform.dropFilesToUpload') }}</p>
    </div>
  </div>
</template>
