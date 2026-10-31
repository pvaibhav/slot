<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'
import type { GameRow } from '@/api/client'

defineProps<{
  games: GameRow[]
  titles?: Map<string, string>
  sortKey: string
  sortDir: 1 | -1
  selectMode?: boolean
  selectedGameIds?: Set<number>
}>()

defineEmits<{
  sort: [key: string]
  'press-game': [id: number, ev: Event]
  'click-game': [id: number, ev: Event]
  'cancel-press': []
}>()

const { t } = useI18n()

const columns = computed<{ key: string; label: string; align: string }[]>(() => [
  { key: 'title', label: t('platform.colTitle'), align: 'text-left' },
  { key: 'saves', label: t('platform.colSaves'), align: 'text-center' },
  { key: 'states', label: t('platform.colStates'), align: 'text-center' },
  { key: 'guides', label: t('platform.colGuides'), align: 'text-center' },
])
</script>

<template>
  <div class="rounded-xl border border-border overflow-hidden">
    <table class="w-full border-collapse text-sm">
      <thead>
        <tr>
          <th v-if="selectMode" class="w-10 px-3 py-2.5 border-b border-border" />
          <th
            v-for="col in columns"
            :key="col.key"
            class="px-3 py-2.5 text-xs font-semibold uppercase tracking-wide text-foreground/60 border-b border-border cursor-pointer select-none hover:text-foreground"
            :class="[col.align, col.key === 'saves' || col.key === 'states' || col.key === 'guides' ? 'w-20' : '']"
            @click="$emit('sort', col.key)"
          >
            {{ col.label }}<span v-if="sortKey === col.key"> {{ sortDir === 1 ? '▴' : '▾' }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="(g, idx) in games"
          :key="g.id"
          class="cursor-pointer select-none"
          :class="selectMode && selectedGameIds?.has(g.id) ? 'bg-accent/15' : [idx % 2 === 1 ? 'bg-muted/20' : '', 'hover:bg-muted/50']"
          @click="$emit('click-game', g.id, $event)"
          @pointerdown="$emit('press-game', g.id, $event)"
          @pointerup="$emit('cancel-press')"
          @pointerleave="$emit('cancel-press')"
          @pointercancel="$emit('cancel-press')"
          @contextmenu.prevent
        >
          <td v-if="selectMode" class="px-3 py-2.5 text-center">
            <span class="inline-flex h-4 w-4 items-center justify-center rounded border-2 border-foreground/40" :class="selectedGameIds?.has(g.id) ? 'bg-accent border-accent' : ''">
              <Check v-if="selectedGameIds?.has(g.id)" class="h-3 w-3 text-accent-foreground" />
            </span>
          </td>
          <td class="px-3 py-2.5 font-semibold text-foreground">{{ titles?.get(g.displayName) ?? g.displayName }}</td>
          <td class="px-3 py-2.5 text-center" :class="g.savesCount ? 'text-foreground/80' : 'text-foreground/30'">{{ g.savesCount || '-' }}</td>
          <td class="px-3 py-2.5 text-center" :class="g.statesCount ? 'text-foreground/80' : 'text-foreground/30'">{{ g.statesCount || '-' }}</td>
          <td class="px-3 py-2.5 text-center" :class="g.guidesCount ? 'text-foreground/80' : 'text-foreground/30'">{{ g.guidesCount || '-' }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
