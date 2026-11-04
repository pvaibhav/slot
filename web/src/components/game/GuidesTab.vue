<script setup lang="ts">
import { onMounted, onActivated, onDeactivated, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { listGameGuides, downloadGameGuide, uploadGameGuide, deleteGameGuide, type GameFile } from '@/api/client'
import { confirm } from '@/lib/confirm'
import { formatSize, formatRelativeTime as formatWhen } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import { BookOpen, Upload, Download, Trash2, Eye } from 'lucide-vue-next'

const props = defineProps<{ tag: string; id: number }>()
const router = useRouter()
const { t } = useI18n()

function openGuide(f: GameFile) {
  router.push({ name: 'guide-view', params: { tag: props.tag, id: props.id, file: f.name } })
}

const entries = ref<GameFile[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const busy = ref(false)

const isActive = ref(true)
onActivated(() => { isActive.value = true })
onDeactivated(() => { isActive.value = false })

async function load() {
  loading.value = true
  error.value = null
  try {
    entries.value = await listGameGuides(props.tag, props.id)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('game.loadGuidesFailed')
  } finally {
    loading.value = false
  }
}

const input = ref<HTMLInputElement | null>(null)

async function picked(event: Event) {
  const el = event.target as HTMLInputElement
  const file = el.files?.[0]
  el.value = ''
  if (!file) return
  busy.value = true
  error.value = null
  try {
    await uploadGameGuide(props.tag, props.id, file)
    await load()
  } catch {
    error.value = t('game.uploadFailed')
  } finally {
    busy.value = false
  }
}

async function onDownload(f: GameFile) {
  try {
    await downloadGameGuide(props.tag, props.id, f.name)
  } catch { error.value = t('game.downloadFailed') }
}

async function onDelete(f: GameFile) {
  if (!await confirm({ title: t('game.deleteFileTitle', { name: f.name }), confirmLabel: t('common.delete'), destructive: true })) return
  busy.value = true
  error.value = null
  try {
    await deleteGameGuide(props.tag, props.id, f.name)
    await load()
  } catch {
    error.value = t('game.deleteFailed')
  } finally {
    busy.value = false
  }
}

onMounted(load)
</script>

<template>
  <div class="space-y-3">
    <input ref="input" type="file" class="hidden" @change="picked" />
    <Teleport v-if="isActive" to="#game-tab-actions">
      <Button variant="outline" size="sm" :disabled="busy" @click="input?.click()">
        <Upload class="h-4 w-4 mr-1.5" /> {{ $t('common.upload') }}
      </Button>
    </Teleport>

    <p class="text-sm text-foreground/60">{{ $t('game.guidesTxtOnly') }}</p>

    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('common.loading') }}</p>
    <div v-else-if="error" class="py-8 text-center space-y-2">
      <p class="text-destructive">{{ error }}</p>
      <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
    </div>
    <p v-else-if="!entries.length" class="text-base text-foreground/75 py-8 text-center">{{ $t('game.noGuides') }}</p>

    <div v-else class="space-y-2">
      <div
        v-for="f in entries"
        :key="f.name"
        class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card"
      >
        <div class="flex items-center justify-center h-9 w-9 rounded-lg bg-accent/15 shrink-0">
          <BookOpen class="h-4 w-4 text-accent" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-base font-semibold text-foreground truncate">{{ f.name }}</div>
          <div class="text-sm text-foreground/60">
            {{ formatSize(f.size) }}<template v-if="formatWhen(f.modified)"> &middot; {{ formatWhen(f.modified) }}</template>
          </div>
        </div>
        <div class="flex items-center gap-1.5 shrink-0">
          <button
            class="p-2 rounded text-foreground/70 hover:bg-muted hover:text-foreground disabled:opacity-40"
            :disabled="busy"
            :title="$t('common.view')"
            @click="openGuide(f)"
          >
            <Eye class="h-4 w-4" />
          </button>
          <button
            class="p-2 rounded text-foreground/70 hover:bg-muted hover:text-foreground disabled:opacity-40"
            :disabled="busy"
            :title="$t('common.download')"
            @click="onDownload(f)"
          >
            <Download class="h-4 w-4" />
          </button>
          <button
            class="p-2 rounded text-foreground/70 hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"
            :disabled="busy"
            :title="$t('common.delete')"
            @click="onDelete(f)"
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
