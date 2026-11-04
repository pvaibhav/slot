<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { listFiles, uploadFiles, createFolder, deleteFile, moveFile, downloadToDisk, type FileEntry } from '@/api/client'
import { platformName } from '@/api/platforms'
import { confirm } from '@/lib/confirm'
import { formatSize, isPreviewable } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import Input from '@/components/ui/Input.vue'
import Progress from '@/components/ui/Progress.vue'
import Breadcrumbs, { type Crumb } from '@/components/layout/Breadcrumbs.vue'
import { ArrowLeft, Upload, File as FileIcon, Folder, FolderPlus, CheckCircle, Trash2, MoveRight, Pencil, ChevronRight, XCircle, Ban, Loader2, Download, Eye } from 'lucide-vue-next'

const props = defineProps<{
  resource: string
  tag?: string
}>()

const router = useRouter()
const route = useRoute()
const { t } = useI18n()

const entries = ref<FileEntry[]>([])
const loading = ref(true)
const uploading = ref(false)
const uploadProgress = ref(0)
const uploadResult = ref<string[]>([])
const abortUpload = ref<(() => void) | null>(null)
const fileInput = ref<HTMLInputElement>()
const dragOver = ref(false)
const uploadFileNames = ref('')
const uploadCurrentIndex = ref(0)
const uploadTotal = ref(0)
const uploadCancelled = ref(false)
type UploadStatus = 'pending' | 'uploading' | 'done' | 'failed' | 'cancelled'
const uploadQueue = ref<{ name: string; status: UploadStatus }[]>([])
const showNewFolder = ref(false)
const newFolderName = ref('')
const creatingFolder = ref(false)
const deleting = ref<string | null>(null)
const downloading = ref<string | null>(null)
const movingEntry = ref<string | null>(null)
const moveBrowsePath = ref<string[]>([])
const moveFolders = ref<string[]>([])
const moveLoading = ref(false)
const moveError = ref('')
const renamingEntry = ref<string | null>(null)
const renameValue = ref('')
const renameError = ref('')
const subpath = computed<string[]>(() => {
  const p = route.query.path as string | undefined
  if (!p) return []
  return p.split('/').filter(Boolean)
})

const apiSegments = computed(() => [props.tag, ...subpath.value].filter(Boolean) as string[])
const atCardRoot = computed(() => props.resource === 'fs' && apiSegments.value.length <= 1)

const resourceLabel = computed(() => {
  if (props.resource === 'fs') return (route.query.label as string) || props.tag || 'fs'
  const labels: Record<string, string> = {
    roms: t('browse.resourceLabel.roms'), art: t('browse.resourceLabel.art'), saves: t('browse.resourceLabel.saves'),
    bios: t('browse.resourceLabel.bios'), wallpapers: t('browse.resourceLabel.wallpapers'),
    guides: t('browse.resourceLabel.guides'), overlays: t('browse.resourceLabel.overlays'),
  }
  return labels[props.resource] ?? props.resource
})

const breadcrumbs = computed(() => {
  const crumbs: { label: string; path: string[] }[] = []
  subpath.value.forEach((seg, i) => {
    crumbs.push({ label: seg, path: subpath.value.slice(0, i + 1) })
  })
  return crumbs
})

async function load(showLoading = true) {
  if (showLoading) loading.value = true
  try {
    const data = await listFiles(props.resource, ...apiSegments.value)
    entries.value = data.entries
  } catch {
    entries.value = []
  } finally {
    loading.value = false
  }
}

async function reload() {
  return load(false)
}

function routeFor(pathSegments: string[]) {
  const label = route.query.label as string | undefined
  const baseQuery = label ? { label } : {}
  const query = pathSegments.length ? { ...baseQuery, path: pathSegments.join('/') } : (label ? baseQuery : undefined)
  return {
    name: props.tag ? 'browse' : 'browse-flat',
    params: props.tag ? { resource: props.resource, tag: props.tag } : { resource: props.resource },
    query,
  }
}

function navigateTo(pathSegments: string[]) {
  router.push(routeFor(pathSegments))
}

const tagLabel = computed(() => (props.tag ? platformName(props.tag) : ''))

const rootLabel = computed(() => props.resource === 'fs'
  ? resourceLabel.value
  : `${resourceLabel.value}${props.tag ? `/${tagLabel.value}` : ''}`)

const crumbs = computed<Crumb[]>(() => {
  const items: Crumb[] = [{ label: rootLabel.value, to: routeFor([]) }]
  breadcrumbs.value.forEach((c, i) => {
    items.push({ label: c.label, to: i < breadcrumbs.value.length - 1 ? routeFor(c.path) : undefined })
  })
  return items
})

function openView(name: string) {
  router.push({
    name: 'file-view',
    query: {
      resource: props.resource,
      ...(props.tag ? { tag: props.tag } : {}),
      ...(subpath.value.length ? { path: subpath.value.join('/') } : {}),
      name,
      ...(route.query.label ? { label: route.query.label as string } : {}),
      rlabel: rootLabel.value,
    },
  })
}

function openFolder(name: string) {
  navigateTo([...subpath.value, name])
}

function goBack() {
  if (subpath.value.length > 0) {
    navigateTo(subpath.value.slice(0, -1))
  } else if (props.resource === 'fs') {
    router.push({ name: 'tools-files' })
  } else {
    router.push(props.tag ? { name: 'platform', params: { tag: props.tag } } : { name: 'dashboard', params: { tab: 'customization' } })
  }
}

function triggerUpload() {
  fileInput.value?.click()
}

async function doUpload(files: globalThis.File[]) {
  if (!files.length) return

  uploading.value = true
  uploadCancelled.value = false
  uploadProgress.value = 0
  uploadResult.value = []
  uploadTotal.value = files.length
  uploadCurrentIndex.value = 0
  uploadQueue.value = files.map((f) => ({ name: f.name, status: 'pending' as UploadStatus }))

  const uploaded: string[] = []
  try {
    for (let i = 0; i < files.length; i++) {
      if (uploadCancelled.value) {
        for (let j = i; j < uploadQueue.value.length; j++) {
          uploadQueue.value[j]!.status = 'cancelled'
        }
        break
      }
      const file = files[i]!
      uploadCurrentIndex.value = i + 1
      uploadFileNames.value = file.name
      uploadProgress.value = 0
      uploadQueue.value[i]!.status = 'uploading'

      const { promise, abort } = uploadFiles(
        props.resource,
        apiSegments.value,
        [file],
        (pct) => { uploadProgress.value = pct },
      )
      abortUpload.value = abort
      try {
        const result = await promise
        uploaded.push(...result.files)
        uploadQueue.value[i]!.status = 'done'
      } catch {
        if (uploadCancelled.value) {
          uploadQueue.value[i]!.status = 'cancelled'
          for (let j = i + 1; j < uploadQueue.value.length; j++) {
            uploadQueue.value[j]!.status = 'cancelled'
          }
        } else {
          uploadQueue.value[i]!.status = 'failed'
        }
        break
      } finally {
        abortUpload.value = null
      }
    }
    uploadResult.value = uploaded
    await reload()
  } finally {
    uploading.value = false
    abortUpload.value = null
  }
}

function cancelUpload() {
  uploadCancelled.value = true
  abortUpload.value?.()
}

async function handleDelete(entry: FileEntry) {
  const title = entry.type === 'dir'
    ? t('browse.deleteFolderConfirm', { name: entry.name })
    : t('browse.deleteFileConfirm', { name: entry.name })
  if (!await confirm({ title, confirmLabel: t('common.delete'), destructive: true })) return
  deleting.value = entry.name
  try {
    await deleteFile(props.resource, ...apiSegments.value, entry.name)
    await reload()
  } finally {
    deleting.value = null
  }
}

async function handleDownload(name: string) {
  downloading.value = name
  try {
    await downloadToDisk(props.resource, [...apiSegments.value, name], name)
  } finally {
    downloading.value = null
  }
}

async function loadMoveFolders() {
  moveLoading.value = true
  try {
    const segments = [props.tag, ...moveBrowsePath.value].filter(Boolean) as string[]
    const data = await listFiles(props.resource, ...segments)
    moveFolders.value = data.entries.filter(e => e.type === 'dir').map(e => e.name)
  } catch {
    moveFolders.value = []
  } finally {
    moveLoading.value = false
  }
}

async function startMove(name: string) {
  renamingEntry.value = null
  movingEntry.value = name
  moveBrowsePath.value = [...subpath.value]
  moveError.value = ''
  await loadMoveFolders()
}

function cancelMove() {
  movingEntry.value = null
  moveBrowsePath.value = []
  moveFolders.value = []
  moveError.value = ''
}

async function moveBrowseInto(folder: string) {
  moveBrowsePath.value = [...moveBrowsePath.value, folder]
  await loadMoveFolders()
}

async function moveBrowseUp() {
  moveBrowsePath.value = moveBrowsePath.value.slice(0, -1)
  await loadMoveFolders()
}

async function moveBrowseToRoot() {
  moveBrowsePath.value = []
  await loadMoveFolders()
}

async function confirmMove() {
  if (!movingEntry.value) return
  const fromSegments = [...apiSegments.value, movingEntry.value]
  const destRoot = props.resource === 'fs' ? [] : [props.tag]
  const destPath = [...destRoot, ...moveBrowsePath.value, movingEntry.value].filter(Boolean).join('/')
  try {
    await moveFile(props.resource, fromSegments, destPath)
    cancelMove()
    await reload()
  } catch {
    moveError.value = t('browse.moveFailed')
  }
}

function startRename(name: string) {
  movingEntry.value = null
  renamingEntry.value = name
  renameValue.value = name
  renameError.value = ''
}

function cancelRename() {
  renamingEntry.value = null
  renameValue.value = ''
  renameError.value = ''
}

async function confirmRename() {
  if (!renamingEntry.value || !renameValue.value.trim()) return
  if (renameValue.value.trim() === renamingEntry.value) { cancelRename(); return }
  const fromSegments = [...apiSegments.value, renamingEntry.value]
  const destRoot = props.resource === 'fs' ? [] : [props.tag]
  const destPath = [...destRoot, ...subpath.value, renameValue.value.trim()].filter(Boolean).join('/')
  try {
    await moveFile(props.resource, fromSegments, destPath)
    cancelRename()
    await reload()
  } catch {
    renameError.value = t('browse.renameFailed')
  }
}

async function handleFiles(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  await doUpload(files)
}

function onDrop(event: DragEvent) {
  dragOver.value = false
  doUpload(Array.from(event.dataTransfer?.files ?? []))
}

async function handleCreateFolder() {
  const name = newFolderName.value.trim()
  if (!name) return

  creatingFolder.value = true
  try {
    await createFolder(props.resource, ...apiSegments.value, name)
    newFolderName.value = ''
    showNewFolder.value = false
    openFolder(name)
  } finally {
    creatingFolder.value = false
  }
}

watch(() => route.query.path, () => load())

onMounted(load)
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6 space-y-6">
    <div class="flex flex-wrap items-center gap-3">
      <Button variant="ghost" size="icon" @click="goBack">
        <ArrowLeft class="h-5 w-5" />
      </Button>
      <div class="flex-1 min-w-0">
        <Breadcrumbs :items="crumbs" />
      </div>
      <Button v-if="!['bios', 'wallpapers'].includes(props.resource)" variant="outline" size="sm" class="shrink-0" @click="showNewFolder = !showNewFolder">
        <FolderPlus class="h-4 w-4" />
        {{ $t('common.newFolder') }}
      </Button>
    </div>

    <div v-if="showNewFolder" class="flex items-center gap-2">
      <Input
        v-model="newFolderName"
        :placeholder="$t('common.folderName')"
        class="flex-1"
        @keydown.enter="handleCreateFolder"
      />
      <Button size="sm" :disabled="creatingFolder || !newFolderName.trim()" @click="handleCreateFolder">
        {{ $t('common.create') }}
      </Button>
      <Button variant="ghost" size="sm" @click="showNewFolder = false; newFolderName = ''">
        {{ $t('common.cancel') }}
      </Button>
    </div>

    <div
      class="rounded-xl border-2 border-dashed p-6 text-center transition-all duration-150"
      :class="dragOver ? 'border-accent bg-accent/5' : 'border-border'"
      @dragover.prevent="dragOver = true"
      @dragleave="dragOver = false"
      @drop.prevent="onDrop"
    >
      <div class="space-y-3">
        <Upload class="h-8 w-8 mx-auto text-muted-foreground" />
        <div>
          <p class="text-sm text-muted-foreground">{{ $t('browse.dragFilesHere') }}</p>
          <button
            class="mt-1 text-sm font-medium text-foreground hover:text-muted-foreground cursor-pointer"
            :disabled="uploading"
            @click="triggerUpload"
          >
            {{ $t('browse.browseToUpload') }}
          </button>
          <input ref="fileInput" type="file" multiple class="hidden" @change="handleFiles" />
        </div>
      </div>
      <div v-if="uploading" class="mt-4 space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span class="text-foreground font-medium truncate">
            {{ uploadFileNames }}<span v-if="uploadTotal > 1" class="text-muted-foreground font-normal"> {{ $t('browse.fileCountOfTotal', { current: uploadCurrentIndex, total: uploadTotal }) }}</span>
          </span>
          <span class="font-mono text-muted-foreground ml-2 shrink-0">{{ uploadProgress }}%</span>
        </div>
        <div class="flex items-center gap-3">
          <Progress :value="uploadProgress" class="flex-1" />
          <button
            class="shrink-0 rounded-lg border border-destructive/50 bg-destructive/10 px-3 py-1.5 text-xs font-semibold text-destructive hover:bg-destructive/20"
            @click="cancelUpload"
          >
            {{ $t('common.cancel') }}
          </button>
        </div>
        <div
          v-if="uploadQueue.length > 1"
          class="max-h-64 overflow-y-auto rounded-lg border border-border divide-y divide-border"
        >
          <div
            v-for="item in uploadQueue"
            :key="item.name"
            class="flex items-center gap-2 px-3 py-2 text-xs"
          >
            <CheckCircle v-if="item.status === 'done'" class="h-3.5 w-3.5 shrink-0 text-accent" />
            <Loader2 v-else-if="item.status === 'uploading'" class="h-3.5 w-3.5 shrink-0 text-accent animate-spin" />
            <XCircle v-else-if="item.status === 'failed'" class="h-3.5 w-3.5 shrink-0 text-destructive" />
            <Ban v-else-if="item.status === 'cancelled'" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
            <FileIcon v-else class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
            <span
              class="flex-1 truncate"
              :class="{
                'text-foreground font-medium': item.status === 'uploading' || item.status === 'done',
                'text-muted-foreground': item.status === 'pending' || item.status === 'cancelled',
                'text-destructive': item.status === 'failed',
              }"
            >{{ item.name }}</span>
          </div>
        </div>
      </div>
      <div v-if="uploadResult.length" class="mt-3 flex items-center justify-center gap-2 text-sm text-foreground">
        <CheckCircle class="h-4 w-4" />
        <span>{{ $t('browse.uploaded', uploadResult.length) }}</span>
      </div>
    </div>

    <div v-if="loading" class="text-sm text-muted-foreground py-8 text-center">{{ $t('common.loading') }}</div>
    <div v-else-if="!entries.length" class="text-sm text-muted-foreground py-8 text-center">{{ $t('browse.noFilesYet') }}</div>

    <div v-else class="rounded-xl border border-border overflow-hidden">
      <div
        v-for="(entry, idx) in entries"
        :key="entry.name"
        class="group flex items-center gap-3 px-4 py-3 hover:bg-muted/50"
        :class="[
          idx > 0 ? 'border-t border-border' : '',
          entry.type === 'dir' ? 'cursor-pointer' : '',
        ]"
        @click="entry.type === 'dir' ? openFolder(entry.name) : undefined"
      >
        <Folder v-if="entry.type === 'dir'" class="h-4 w-4 text-accent shrink-0" />
        <FileIcon v-else class="h-4 w-4 text-muted-foreground shrink-0" />
        <span class="flex-1 truncate text-sm">{{ entry.name }}</span>
        <span class="text-xs text-muted-foreground tabular-nums">{{ formatSize(entry.size) }}</span>
        <button
          v-if="entry.type === 'file' && isPreviewable(entry.name)"
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-accent hover:bg-accent/10 transition-colors"
          :title="$t('common.view')"
          @click.stop="openView(entry.name)"
        >
          <Eye class="h-3.5 w-3.5" />
        </button>
        <button
          v-if="props.resource === 'fs' && entry.type === 'file'"
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-accent hover:bg-accent/10 transition-colors"
          :title="$t('common.download')"
          :disabled="downloading === entry.name"
          @click.stop="handleDownload(entry.name)"
        >
          <Loader2 v-if="downloading === entry.name" class="h-3.5 w-3.5 animate-spin" />
          <Download v-else class="h-3.5 w-3.5" />
        </button>
        <button
          v-if="!(entry.type === 'dir' && atCardRoot)"
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-accent hover:bg-accent/10 transition-colors"
          :title="$t('common.rename')"
          @click.stop="startRename(entry.name)"
        >
          <Pencil class="h-3.5 w-3.5" />
        </button>
        <button
          v-if="props.resource !== 'wallpapers' && !(entry.type === 'dir' && atCardRoot)"
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-accent hover:bg-accent/10 transition-colors"
          :title="$t('common.move')"
          @click.stop="startMove(entry.name)"
        >
          <MoveRight class="h-3.5 w-3.5" />
        </button>
        <button
          v-if="!(entry.type === 'dir' && atCardRoot)"
          class="shrink-0 p-1 rounded text-muted-foreground/50 hover:text-destructive hover:bg-destructive/10 transition-colors"
          :title="$t('common.delete')"
          :disabled="deleting === entry.name"
          @click.stop="handleDelete(entry)"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </button>
      </div>
    </div>

    <div v-if="renamingEntry" class="rounded-xl border border-accent/50 bg-card p-4 space-y-3">
      <p class="text-sm">
        {{ $t('browse.renameLabel') }} <span class="font-semibold text-foreground">{{ renamingEntry }}</span>
      </p>
      <Input
        v-model="renameValue"
        :placeholder="$t('browse.newName')"
        @keydown.enter="confirmRename"
        @keydown.escape="cancelRename"
      />
      <p v-if="renameError" class="text-xs text-destructive">{{ renameError }}</p>
      <div class="flex items-center gap-2">
        <Button size="sm" :disabled="!renameValue.trim() || renameValue === renamingEntry" @click="confirmRename">{{ $t('common.rename') }}</Button>
        <Button variant="ghost" size="sm" @click="cancelRename">{{ $t('common.cancel') }}</Button>
      </div>
    </div>

    <div v-if="movingEntry" class="rounded-xl border border-accent/50 bg-card p-4 space-y-3">
      <p class="text-sm">
        {{ $t('browse.moveLabel') }} <span class="font-semibold text-foreground">{{ movingEntry }}</span> {{ $t('browse.moveToSuffix') }}
      </p>

      <div class="flex items-center gap-1 text-sm text-muted-foreground flex-wrap">
        <button class="hover:text-foreground font-medium" @click="moveBrowseToRoot">{{ resourceLabel }}{{ props.tag && props.resource !== 'fs' ? `/${tagLabel}` : '' }}</button>
        <template v-for="(seg, i) in moveBrowsePath" :key="i">
          <ChevronRight class="h-3.5 w-3.5 shrink-0" />
          <button
            class="hover:text-foreground"
            :class="i === moveBrowsePath.length - 1 ? 'text-foreground font-medium' : ''"
            @click="moveBrowsePath = moveBrowsePath.slice(0, i + 1); loadMoveFolders()"
          >
            {{ seg }}
          </button>
        </template>
      </div>

      <div class="rounded-lg border border-border max-h-48 overflow-y-auto">
        <div v-if="moveLoading" class="p-3 text-sm text-muted-foreground text-center">{{ $t('common.loading') }}</div>
        <template v-else>
          <button
            v-if="moveBrowsePath.length > 0"
            class="flex items-center gap-2 w-full px-3 py-2 text-sm text-left hover:bg-muted/50 border-b border-border"
            @click="moveBrowseUp"
          >
            <ArrowLeft class="h-3.5 w-3.5 text-muted-foreground" />
            <span class="text-muted-foreground">..</span>
          </button>
          <div v-if="!moveFolders.length && moveBrowsePath.length === 0" class="p-3 text-sm text-muted-foreground text-center">
            {{ $t('browse.noSubfolders') }}
          </div>
          <button
            v-for="folder in moveFolders"
            :key="folder"
            class="flex items-center gap-2 w-full px-3 py-2 text-sm text-left hover:bg-muted/50 border-t border-border first:border-t-0"
            @click="moveBrowseInto(folder)"
          >
            <Folder class="h-3.5 w-3.5 text-accent shrink-0" />
            <span class="truncate">{{ folder }}</span>
            <ChevronRight class="h-3.5 w-3.5 text-muted-foreground ml-auto shrink-0" />
          </button>
        </template>
      </div>

      <p v-if="moveError" class="text-xs text-destructive">{{ moveError }}</p>
      <div class="flex items-center gap-2">
        <Button size="sm" @click="confirmMove">
          {{ $t('browse.moveHere') }}
        </Button>
        <Button variant="ghost" size="sm" @click="cancelMove">{{ $t('common.cancel') }}</Button>
      </div>
    </div>

  </div>
</template>
