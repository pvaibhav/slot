<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { listVolumes, type Volume } from '@/api/client'
import { formatSize } from '@/lib/format'
import Button from '@/components/ui/Button.vue'
import { ArrowLeft } from 'lucide-vue-next'

const router = useRouter()
const volumes = ref<Volume[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    volumes.value = await listVolumes()
  } finally {
    loading.value = false
  }
})

function openVolume(v: Volume) {
  router.push({
    name: 'browse',
    params: { resource: 'fs', tag: v.id },
    query: { label: v.label },
  })
}

function usedPct(v: Volume): number {
  return v.totalBytes ? Math.round(((v.totalBytes - v.freeBytes) / v.totalBytes) * 100) : 0
}
</script>

<template>
  <div class="mx-auto max-w-[1600px] p-6 space-y-6">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" @click="router.push({ name: 'dashboard', params: { tab: 'tools' } })">
        <ArrowLeft class="h-5 w-5" />
      </Button>
      <h1 class="text-2xl font-bold tracking-tight">{{ $t('tools.fileManager') }}</h1>
    </div>

    <p v-if="loading" class="text-sm text-muted-foreground py-8 text-center">{{ $t('tools.loadingVolumes') }}</p>
    <p v-else-if="!volumes.length" class="text-sm text-muted-foreground py-8 text-center">{{ $t('tools.noVolumes') }}</p>
    <div v-else class="grid grid-cols-1 sm:grid-cols-2 gap-4">
      <button
        v-for="v in volumes"
        :key="v.id"
        class="rounded-xl border border-border bg-card p-6 text-left transition-colors hover:border-accent/50 hover:bg-muted"
        @click="openVolume(v)"
      >
        <h3 class="text-base font-semibold">{{ v.label }}</h3>
        <p class="mt-1 text-sm text-muted-foreground">
          {{ $t('tools.volumeFreeOfTotal', { free: formatSize(v.freeBytes), total: formatSize(v.totalBytes) }) }}
        </p>
        <div class="mt-3 h-1.5 rounded-full bg-muted overflow-hidden">
          <div class="h-full bg-accent" :style="{ width: `${usedPct(v)}%` }" />
        </div>
      </button>
    </div>
  </div>
</template>
