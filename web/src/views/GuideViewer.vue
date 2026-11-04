<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { gameGuideText, getGame, downloadGameGuide, type GameDetail } from '@/api/client'
import { platformName, platformRoute } from '@/api/platforms'
import Breadcrumbs, { type Crumb } from '@/components/layout/Breadcrumbs.vue'
import Button from '@/components/ui/Button.vue'
import { Download, Loader2 } from 'lucide-vue-next'

const props = defineProps<{ tag: string; id: string; file: string }>()

const { t } = useI18n()
const gameId = computed(() => Number(props.id))
const game = ref<GameDetail | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)
const text = ref('')
const downloadError = ref('')

const crumbs = computed<Crumb[]>(() => [
  { label: t('browse.guideViewer.platforms'), to: { name: 'dashboard' } },
  { label: platformName(props.tag), to: platformRoute(props.tag) },
  { label: game.value?.displayName ?? t('browse.guideViewer.game'), to: { name: 'game', params: { tag: props.tag, id: props.id, tab: 'guides' } } },
  { label: props.file },
])

async function load() {
  loading.value = true
  error.value = null
  getGame(props.tag, gameId.value).then(g => { game.value = g }).catch(() => {})
  try {
    text.value = await gameGuideText(props.tag, gameId.value, props.file)
  } catch (e) {
    error.value = e instanceof Error ? e.message : t('browse.guideViewer.failedToLoad')
  } finally {
    loading.value = false
  }
}

async function onDownload() {
  downloadError.value = ''
  try {
    await downloadGameGuide(props.tag, gameId.value, props.file)
  } catch {
    downloadError.value = t('browse.guideViewer.downloadFailed')
  }
}

onMounted(load)
watch(() => props.file, load)
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
      <Loader2 class="h-5 w-5 animate-spin" /> {{ $t('browse.guideViewer.loadingGuide') }}
    </div>

    <div v-else-if="error" class="py-16 text-center space-y-3">
      <p class="text-destructive">{{ error }}</p>
      <div class="flex items-center justify-center gap-2">
        <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
        <Button variant="outline" size="sm" @click="onDownload">{{ $t('browse.guideViewer.downloadInstead') }}</Button>
      </div>
    </div>

    <pre
      v-else
      class="rounded-xl bg-card border border-border p-5 text-sm leading-relaxed text-foreground/90 whitespace-pre-wrap break-words font-mono overflow-x-auto"
    >{{ text }}</pre>
  </div>
</template>
