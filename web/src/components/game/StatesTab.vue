<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  listGameStates, deleteGameState, downloadGameState, gameStateThumbBlob,
  type CoreStates,
} from '@/api/client'
import { confirm } from '@/lib/confirm'
import { formatSize, stripExtension } from '@/lib/format'
import { groupStates, stampLabel, RESUME_STAMP } from '@/lib/states'
import Button from '@/components/ui/Button.vue'
import { Download, Trash2, History } from 'lucide-vue-next'

const props = defineProps<{ tag: string; id: number; romName: string }>()
const { t } = useI18n()

const cores = ref<CoreStates[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const busy = ref(false)
const thumbs = ref<Map<string, string>>(new Map())
const rowError = ref<Map<string, string>>(new Map())

const groups = computed(() => groupStates(cores.value))
const visibleGroups = computed(() => (loading.value || error.value ? [] : groups.value))
const romBase = computed(() => stripExtension(props.romName))

function key(core: string, stamp: string): string {
  return `${core}/${stamp}`
}

function fileName(core: string, stamp: string): string {
  return stamp === RESUME_STAMP ? `${romBase.value}.${core}.resume.state` : `${romBase.value}.${core}.${stamp}.state`
}

function clearThumbs() {
  for (const url of thumbs.value.values()) URL.revokeObjectURL(url)
  thumbs.value = new Map()
}

async function load(silent = false) {
  if (!silent) loading.value = true
  error.value = null
  rowError.value = new Map()
  try {
    cores.value = await listGameStates(props.tag, props.id)
    const next = new Map<string, string>()
    for (const group of groupStates(cores.value)) {
      for (const s of group.states) {
        const url = await gameStateThumbBlob(props.tag, props.id, group.id, s.stamp)
        if (url) next.set(key(group.id, s.stamp), url)
      }
    }
    const previous = thumbs.value
    thumbs.value = next
    for (const url of previous.values()) URL.revokeObjectURL(url)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('game.loadStatesFailed')
  } finally {
    if (!silent) loading.value = false
  }
}

async function onDownload(core: string, stamp: string) {
  rowError.value.delete(key(core, stamp))
  try {
    await downloadGameState(props.tag, props.id, core, stamp, fileName(core, stamp))
  } catch {
    rowError.value.set(key(core, stamp), t('game.downloadFailed'))
  }
}

async function onDelete(core: string, stamp: string) {
  const resume = stamp === RESUME_STAMP
  const ok = await confirm({
    title: resume ? t('game.deleteResumeTitle') : t('game.deleteStateTitle'),
    body: resume ? t('game.deleteResumeBody') : undefined,
    confirmLabel: t('common.delete'),
    destructive: true,
  })
  if (!ok) return
  busy.value = true
  rowError.value.delete(key(core, stamp))
  try {
    await deleteGameState(props.tag, props.id, core, stamp)
    await load(true)
  } catch {
    rowError.value.set(key(core, stamp), t('game.stateDeleteFailed'))
  } finally {
    busy.value = false
  }
}

onMounted(load)
onBeforeUnmount(clearThumbs)
</script>

<template>
  <div class="space-y-6">
    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('common.loading') }}</p>
    <div v-else-if="error" class="py-8 text-center space-y-2">
      <p class="text-destructive">{{ error }}</p>
      <Button variant="outline" size="sm" @click="load()">{{ $t('common.retry') }}</Button>
    </div>
    <p v-else-if="!groups.length" class="text-base text-foreground/75 py-8 text-center">{{ $t('game.noStates') }}</p>

    <section v-for="group in visibleGroups" :key="group.id" class="space-y-3" :data-core="group.id">
      <h2 class="text-sm font-semibold uppercase tracking-wider text-foreground/60">{{ group.core }}</h2>

      <div
        v-if="group.resume"
        class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card"
      >
        <div class="flex items-center justify-center h-9 w-9 rounded-lg bg-accent/15 shrink-0">
          <History class="h-4 w-4 text-accent" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-base font-semibold text-foreground">{{ $t('game.resumeState') }}</div>
          <p v-if="rowError.get(key(group.id, RESUME_STAMP))" class="text-sm text-destructive">{{ rowError.get(key(group.id, RESUME_STAMP)) }}</p>
        </div>
        <div class="flex gap-1.5 shrink-0">
          <button
            class="p-2 rounded text-foreground/70 hover:bg-muted hover:text-foreground disabled:opacity-40"
            :disabled="busy"
            :title="$t('common.download')"
            @click="onDownload(group.id, RESUME_STAMP)"
          >
            <Download class="h-4 w-4" />
          </button>
          <button
            class="p-2 rounded text-foreground/70 hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"
            :disabled="busy"
            :title="$t('common.delete')"
            @click="onDelete(group.id, RESUME_STAMP)"
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </div>

      <div v-if="group.states.length" class="grid grid-cols-2 sm:grid-cols-3 gap-4">
        <div
          v-for="s in group.states"
          :key="s.stamp"
          class="bg-card border border-border rounded-lg p-2.5"
        >
          <div
            class="bg-surface-sunken rounded-md overflow-hidden flex items-center justify-center"
            style="aspect-ratio: 10 / 9"
          >
            <img
              v-if="thumbs.get(key(group.id, s.stamp))"
              :src="thumbs.get(key(group.id, s.stamp))"
              :alt="stampLabel(s.stamp)"
              class="h-full w-full object-cover"
            />
            <span v-else class="text-sm text-muted-foreground">{{ $t('game.noPreview') }}</span>
          </div>
          <div class="text-center pt-2.5">
            <div class="text-sm font-bold text-foreground">{{ stampLabel(s.stamp) }}</div>
            <div class="text-sm text-muted-foreground mt-0.5">{{ formatSize(s.size) }}</div>
          </div>
          <div class="flex justify-center gap-1.5 pt-2.5">
            <button
              class="p-2 rounded text-foreground/70 hover:bg-muted hover:text-foreground disabled:opacity-40"
              :disabled="busy"
              :title="$t('common.download')"
              @click="onDownload(group.id, s.stamp)"
            >
              <Download class="h-5 w-5" />
            </button>
            <button
              class="p-2 rounded text-foreground/70 hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"
              :disabled="busy"
              :title="$t('common.delete')"
              @click="onDelete(group.id, s.stamp)"
            >
              <Trash2 class="h-5 w-5" />
            </button>
          </div>
          <p v-if="rowError.get(key(group.id, s.stamp))" class="text-sm text-destructive text-center pt-1">{{ rowError.get(key(group.id, s.stamp)) }}</p>
        </div>
      </div>
    </section>
  </div>
</template>
