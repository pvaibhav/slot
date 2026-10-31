<script setup lang="ts">
import { useRouter } from 'vue-router'
import { FolderOpen } from 'lucide-vue-next'

const router = useRouter()
</script>

<template>
  <div class="space-y-4 flex-1">
    <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
      <button
        type="button"
        class="text-left rounded-xl border border-border bg-card p-5 cursor-pointer hover:ring-2 hover:ring-primary hover:ring-offset-2 hover:ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background"
        @click="router.push({ name: 'tools-files' })"
      >
        <div class="flex items-center gap-3">
          <div class="flex items-center justify-center h-10 w-10 rounded-lg bg-muted">
            <FolderOpen class="h-5 w-5 text-accent" />
          </div>
          <span class="font-semibold">{{ $t('tools.fileManager') }}</span>
        </div>
      </button>
    </div>
  </div>
</template>
