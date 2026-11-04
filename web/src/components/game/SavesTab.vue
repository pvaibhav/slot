<script setup lang="ts">
import { onMounted, onActivated, onDeactivated, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { listGameSaves, downloadGameSave, uploadGameSave, deleteGameSave, type GameFile } from '@/api/client'
import { confirm } from '@/lib/confirm'
import { formatSize, formatRelativeTime as formatWhen } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import { saveWarning } from '@/lib/saveWarning'
import { Save, Upload, Download, Trash2, TriangleAlert } from 'lucide-vue-next'

const props = defineProps<{ tag: string; id: number }>()
const emit = defineEmits<{ changed: [] }>()
const { t } = useI18n()

const entries = ref<GameFile[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const busy = ref(false)
const warning = ref<string | null>(null)
const input = ref<HTMLInputElement | null>(null)

const isActive = ref(true)
onActivated(() => { isActive.value = true })
onDeactivated(() => { isActive.value = false })

async function load() {
  loading.value = true
  error.value = null
  try {
    entries.value = await listGameSaves(props.tag, props.id)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('game.loadSavesFailed')
  } finally {
    loading.value = false
  }
}

async function picked(event: Event) {
  const el = event.target as HTMLInputElement
  const file = el.files?.[0]
  el.value = ''
  if (!file) return
  busy.value = true
  error.value = null
  try {
    const result = await uploadGameSave(props.tag, props.id, file)
    warning.value = saveWarning(result.resumeCores)
    await load()
    emit('changed')
  } catch {
    error.value = t('game.uploadFailed')
  } finally {
    busy.value = false
  }
}

async function onDownload(f: GameFile) {
  try {
    await downloadGameSave(props.tag, props.id, f.name)
  } catch { error.value = t('game.downloadFailed') }
}

async function onDelete(f: GameFile) {
  if (!await confirm({ title: t('game.deleteFileTitle', { name: f.name }), confirmLabel: t('common.delete'), destructive: true })) return
  busy.value = true
  error.value = null
  try {
    await deleteGameSave(props.tag, props.id, f.name)
    await load()
    emit('changed')
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

    <div
      v-if="warning"
      class="flex items-start gap-3 rounded-lg border border-warn/50 bg-warn/10 px-4 py-3 text-sm text-foreground"
      role="status"
    >
      <TriangleAlert class="h-4 w-4 mt-0.5 shrink-0 text-warn" />
      <span>{{ warning }}</span>
    </div>

    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('common.loading') }}</p>
    <div v-else-if="error" class="py-8 text-center space-y-2">
      <p class="text-destructive">{{ error }}</p>
      <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
    </div>
    <p v-else-if="!entries.length" class="text-base text-foreground/75 py-8 text-center">{{ $t('game.noSaves') }}</p>

    <div v-else class="space-y-2">
      <div
        v-for="f in entries"
        :key="f.name"
        class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card"
      >
        <div class="flex items-center justify-center h-9 w-9 rounded-lg bg-accent/15 shrink-0">
          <Save class="h-4 w-4 text-accent" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-base font-semibold text-foreground truncate">{{ f.name }}</div>
          <div class="text-sm text-foreground/60">
            {{ formatSize(f.size) }}<template v-if="formatWhen(f.modified)"> &middot; {{ formatWhen(f.modified) }}</template>
          </div>
        </div>
        <div class="flex gap-1.5 shrink-0">
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
