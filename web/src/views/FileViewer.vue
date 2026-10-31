<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { RouteLocationRaw } from 'vue-router'
import { fileBlob, downloadToDisk } from '@/api/client'
import Breadcrumbs, { type Crumb } from '@/components/layout/Breadcrumbs.vue'
import FilePreview from '@/components/FilePreview.vue'
import Button from '@/components/ui/Button.vue'
import { Download, Loader2 } from 'lucide-vue-next'

const props = defineProps<{
  resource: string
  tag?: string
  path?: string
  name: string
  label?: string
  rlabel?: string
}>()

const loading = ref(true)
const error = ref<string | null>(null)
const blobUrl = ref<string | null>(null)
const contentType = ref('')
const downloadError = ref('')
const { t } = useI18n()

const pathSegs = computed(() => (props.path ? props.path.split('/').filter(Boolean) : []))
const segments = computed(() => [props.tag, ...pathSegs.value, props.name].filter(Boolean) as string[])

function listRoute(segs: string[]): RouteLocationRaw {
  const query: Record<string, string> = {}
  if (props.label) query.label = props.label
  if (segs.length) query.path = segs.join('/')
  return {
    name: props.tag ? 'browse' : 'browse-flat',
    params: props.tag ? { resource: props.resource, tag: props.tag } : { resource: props.resource },
    query: Object.keys(query).length ? query : undefined,
  }
}

const crumbs = computed<Crumb[]>(() => {
  const items: Crumb[] = [{ label: props.rlabel || props.resource, to: listRoute([]) }]
  pathSegs.value.forEach((seg, i) => {
    items.push({ label: seg, to: listRoute(pathSegs.value.slice(0, i + 1)) })
  })
  items.push({ label: props.name })
  return items
})

async function load() {
  loading.value = true
  error.value = null
  if (blobUrl.value) { URL.revokeObjectURL(blobUrl.value); blobUrl.value = null }
  if (!props.resource || !props.name) {
    error.value = t('browse.fileViewer.nothingToPreview')
    loading.value = false
    return
  }
  try {
    const { url, type } = await fileBlob(props.resource, ...segments.value)
    blobUrl.value = url
    contentType.value = type
  } catch (e) {
    error.value = e instanceof Error ? e.message : t('browse.fileViewer.failedToLoad')
  } finally {
    loading.value = false
  }
}

async function onDownload() {
  downloadError.value = ''
  try {
    await downloadToDisk(props.resource, segments.value, props.name)
  } catch {
    downloadError.value = t('browse.fileViewer.downloadFailed')
  }
}

onMounted(load)
watch(() => [props.resource, props.tag, props.path, props.name], load)
onBeforeUnmount(() => { if (blobUrl.value) URL.revokeObjectURL(blobUrl.value) })
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6 space-y-4">
    <div class="flex items-center justify-between gap-3">
      <Breadcrumbs :items="crumbs" />
      <div class="flex items-center gap-2 shrink-0">
        <span v-if="downloadError" class="text-xs text-destructive">{{ downloadError }}</span>
        <Button variant="outline" size="sm" @click="onDownload">
          <Download class="h-4 w-4 mr-1.5" /> {{ $t('common.download') }}
        </Button>
      </div>
    </div>

    <div v-if="loading" class="flex items-center justify-center gap-2 py-16 text-muted-foreground">
      <Loader2 class="h-5 w-5 animate-spin" /> {{ $t('browse.fileViewer.loadingFile') }}
    </div>

    <div v-else-if="error" class="py-16 text-center space-y-3">
      <p class="text-destructive">{{ error }}</p>
      <div class="flex items-center justify-center gap-2">
        <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
        <Button variant="outline" size="sm" @click="onDownload">{{ $t('browse.fileViewer.downloadInstead') }}</Button>
      </div>
    </div>

    <FilePreview v-else-if="blobUrl" :url="blobUrl" :type="contentType" :name="props.name" />
  </div>
</template>
