<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { deleteFile, downloadToDisk, listFiles, uploadFiles } from '@/api/client'
import { biosFile, platformName } from '@/api/platforms'
import { confirm } from '@/lib/confirm'
import { formatSize } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import { Cpu, Download, Trash2 } from 'lucide-vue-next'

const props = defineProps<{ tag: string }>()

const { t } = useI18n()
const name = computed(() => biosFile(props.tag))
const platform = computed(() => platformName(props.tag))
const size = ref<number | null>(null)
const busy = ref(false)
const dragOver = ref(false)
const error = ref<string | null>(null)
const input = ref<HTMLInputElement>()

async function load() {
  size.value = (await listFiles('bios')).entries.find(e => e.name === name.value)?.size ?? null
}

async function run(job: () => Promise<unknown>, failed: string) {
  busy.value = true
  error.value = null
  try {
    await job()
    await load()
  } catch {
    error.value = failed
  } finally {
    busy.value = false
  }
}

function store(file: File | undefined) {
  if (!file) return
  const named = new File([file], name.value, { type: file.type })
  run(() => uploadFiles('bios', [], [named]).promise, t('fileSlot.uploadFailed'))
}

function picked(event: Event) {
  const el = event.target as HTMLInputElement
  const file = el.files?.[0]
  el.value = ''
  store(file)
}

function dropped(event: DragEvent) {
  dragOver.value = false
  store(event.dataTransfer?.files[0])
}

async function remove() {
  const title = t('browse.deleteFileConfirm', { name: name.value })
  if (!await confirm({ title, confirmLabel: t('common.delete'), destructive: true })) return
  run(() => deleteFile('bios', name.value), t('fileSlot.deleteFailed'))
}

function download() {
  run(() => downloadToDisk('bios', [name.value], name.value), t('fileSlot.downloadFailed'))
}

onMounted(() => run(async () => {}, t('fileSlot.loadFailed')))
</script>

<template>
  <div
    class="space-y-3"
    @dragover.prevent="dragOver = true"
    @dragleave="dragOver = false"
    @drop.prevent="dropped"
  >
    <input ref="input" type="file" accept=".bin,.rom" class="hidden" @change="picked" />

    <div
      v-if="size !== null"
      class="flex flex-wrap items-center gap-4 rounded-xl border bg-card p-5 transition-colors"
      :class="dragOver ? 'border-accent' : 'border-border'"
    >
      <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-muted">
        <Cpu class="h-6 w-6 text-foreground" />
      </div>
      <div class="min-w-0 flex-1 basis-[calc(100%-4rem)] sm:basis-0">
        <p class="font-semibold">{{ $t('platform.biosInstalled', { name: platform }) }}</p>
        <p class="truncate text-sm text-muted-foreground">{{ name }} ({{ formatSize(size) }})</p>
      </div>
      <div class="ml-auto flex items-center gap-2">
        <Button variant="ghost" size="icon" :disabled="busy" :title="$t('fileSlot.download')" @click="download"><Download class="h-4 w-4" /></Button>
        <Button variant="ghost" size="icon" :disabled="busy" :title="$t('common.delete')" @click="remove"><Trash2 class="h-4 w-4" /></Button>
        <Button variant="outline" size="sm" :disabled="busy" @click="input?.click()">{{ $t('platform.biosReplace') }}</Button>
      </div>
    </div>

    <div
      v-else
      class="rounded-xl border-2 border-dashed p-10 text-center transition-all duration-150"
      :class="dragOver ? 'border-accent bg-accent/5' : 'border-border'"
    >
      <Cpu class="h-10 w-10 mx-auto text-muted-foreground" />
      <p class="mt-4 font-semibold">{{ $t('platform.biosMissing', { name: platform }) }}</p>
      <p class="mt-1 text-sm text-muted-foreground">{{ $t('browse.dragFilesHere') }}</p>
      <button
        class="mt-1 text-sm font-medium text-foreground hover:text-muted-foreground cursor-pointer"
        :disabled="busy"
        @click="input?.click()"
      >
        {{ $t('browse.browseToUpload') }}
      </button>
    </div>

    <p class="text-sm text-muted-foreground">{{ $t('platform.biosNote') }}</p>
    <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
  </div>
</template>
