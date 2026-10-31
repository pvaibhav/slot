<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { listGameRoms, downloadGameRomFile, type GameRomFile } from '@/api/client'
import { formatSize } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import { Gamepad2, Download } from 'lucide-vue-next'

const props = defineProps<{ tag: string; id: number }>()
const { t } = useI18n()

const files = ref<GameRomFile[]>([])
const loading = ref(true)
const error = ref<string | null>(null)

async function load() {
  loading.value = true
  error.value = null
  try {
    files.value = await listGameRoms(props.tag, props.id)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : t('game.loadRomsFailed')
  } finally {
    loading.value = false
  }
}

async function onDownload(f: GameRomFile) {
  try {
    await downloadGameRomFile(props.tag, props.id, f.path, f.name)
  } catch { error.value = t('game.downloadFailed') }
}

onMounted(load)
</script>

<template>
  <div class="space-y-3">
    <p v-if="loading" class="text-base text-foreground/75 py-8 text-center">{{ $t('common.loading') }}</p>
    <div v-else-if="error" class="py-8 text-center space-y-2">
      <p class="text-destructive">{{ error }}</p>
      <Button variant="outline" size="sm" @click="load">{{ $t('common.retry') }}</Button>
    </div>
    <p v-else-if="!files.length" class="text-base text-foreground/75 py-8 text-center">{{ $t('game.noRoms') }}</p>

    <div v-else class="space-y-2">
      <div
        v-for="f in files"
        :key="f.path"
        class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card"
      >
        <div class="flex items-center justify-center h-9 w-9 rounded-lg bg-accent/15 shrink-0">
          <Gamepad2 class="h-4 w-4 text-accent" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-base font-semibold text-foreground truncate">{{ f.name }}</div>
          <div class="text-sm text-foreground/60">{{ formatSize(f.size) || '-' }}</div>
        </div>
        <button
          class="p-2 rounded text-foreground/70 hover:bg-muted hover:text-foreground shrink-0"
          :title="$t('common.download')"
          @click="onDownload(f)"
        >
          <Download class="h-4 w-4" />
        </button>
      </div>
    </div>
  </div>
</template>
