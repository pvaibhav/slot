<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { buildPath, deleteFile, imageBlob, listFiles, uploadFiles } from '@/api/client'
import { confirm } from '@/lib/confirm'
import Breadcrumbs, { type Crumb } from '@/components/layout/Breadcrumbs.vue'
import Button from '@/components/ui/Button.vue'
import { ArrowLeft, Upload } from 'lucide-vue-next'

const FILE = 'bootlogo.bmp'
const FOLDER = ['card', 'Config']
const PANEL_WIDTH = 720
const SLOT_GROUND = '#0b0e11'
const DOCS = 'https://slot-cfw.fyi/guide/customizing/#boot-logo'

const { t } = useI18n()
const router = useRouter()

const crumbs = computed<Crumb[]>(() => [{ label: t('dashboard.bootLogo') }])
const custom = ref<string | null>(null)
const checked = ref(false)
const width = ref<number | null>(null)
const busy = ref(false)
const dragOver = ref(false)
const error = ref<string | null>(null)
const input = ref<HTMLInputElement>()

const shown = computed(() => (checked.value ? custom.value ?? '/bootlogo.bmp' : undefined))
const shownWidth = computed(() =>
  width.value ? `${Math.min(100, (width.value / PANEL_WIDTH) * 100)}%` : undefined,
)

function clearCustom() {
  if (custom.value) URL.revokeObjectURL(custom.value)
  custom.value = null
}

async function load() {
  const found = (await listFiles('fs', ...FOLDER)).entries.some(e => e.name === FILE)
  const next = found ? await imageBlob(buildPath('fs', ...FOLDER, FILE), new AbortController().signal) : null
  clearCustom()
  custom.value = next
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
    checked.value = true
    busy.value = false
  }
}

function store(file: File | undefined) {
  if (!file) return
  const named = new File([file], FILE, { type: file.type })
  run(() => uploadFiles('fs', FOLDER, [named]).promise, t('fileSlot.uploadFailed'))
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

async function restore() {
  const title = t('dashboard.bootLogoRestore')
  if (!await confirm({ title, confirmLabel: t('dashboard.bootLogoRestore') })) return
  run(() => deleteFile('fs', ...FOLDER, FILE), t('fileSlot.deleteFailed'))
}

function measured(event: Event) {
  width.value = (event.target as HTMLImageElement).naturalWidth
}

function goBack() {
  router.push({ name: 'dashboard', params: { tab: 'customization' } })
}

onMounted(() => run(async () => {}, t('fileSlot.loadFailed')))
onUnmounted(clearCustom)
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6 space-y-6">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" @click="goBack">
        <ArrowLeft class="h-5 w-5" />
      </Button>
      <div class="flex-1 min-w-0">
        <Breadcrumbs :items="crumbs" />
      </div>
    </div>

    <div class="mx-auto w-full max-w-2xl space-y-4">
      <div
        class="aspect-[3/2] flex items-center justify-center overflow-hidden rounded-xl ring-1 transition-all"
        :style="{ background: custom || !checked ? '#000' : SLOT_GROUND }"
        :class="dragOver ? 'ring-2 ring-accent' : 'ring-border'"
        @dragover.prevent="dragOver = true"
        @dragleave="dragOver = false"
        @drop.prevent="dropped"
      >
        <img v-if="shown" :src="shown" :alt="$t('dashboard.bootLogo')" :style="{ width: shownWidth, imageRendering: 'pixelated' }" @load="measured" />
      </div>
      <div class="flex items-center justify-center gap-2">
        <Button v-if="custom" variant="ghost" size="sm" :disabled="busy" @click="restore">{{ $t('dashboard.bootLogoRestore') }}</Button>
        <Button size="sm" :disabled="busy" @click="input?.click()">
          <Upload class="h-3.5 w-3.5" />{{ $t('dashboard.bootLogoReplace') }}
        </Button>
      </div>
      <input ref="input" type="file" accept=".bmp,image/bmp" class="hidden" @change="picked" />
      <p class="text-center text-sm text-muted-foreground">{{ $t('dashboard.bootLogoNote') }}</p>
      <p class="text-center text-sm">
        <a :href="DOCS" target="_blank" rel="noopener" class="text-muted-foreground underline hover:text-foreground">{{ $t('dashboard.bootLogoDocs') }}</a>
      </p>
      <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
    </div>
  </div>
</template>
