<script setup lang="ts">
import { ref, onMounted, onUnmounted, type Component } from 'vue'
import { ChevronDown } from 'lucide-vue-next'
import Button from '@/components/ui/Button.vue'

export interface DropdownItem {
  label: string
  icon?: Component
  danger?: boolean
  onSelect: () => void
}

const props = defineProps<{
  items: DropdownItem[]
}>()

const open = ref(false)
const containerRef = ref<HTMLElement | null>(null)

function toggle() {
  open.value = !open.value
}

function select(item: DropdownItem) {
  item.onSelect()
  open.value = false
}

function onOutsideClick(e: MouseEvent) {
  if (containerRef.value && !containerRef.value.contains(e.target as Node)) {
    open.value = false
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    open.value = false
  }
}

onMounted(() => {
  document.addEventListener('mousedown', onOutsideClick)
  document.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  document.removeEventListener('mousedown', onOutsideClick)
  document.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div ref="containerRef" class="relative inline-block">
    <Button
      v-if="items.length === 1"
      variant="outline"
      :class="items[0]!.danger ? 'text-destructive' : 'text-foreground'"
      @click="items[0]!.onSelect()"
    >
      <component :is="items[0]!.icon" v-if="items[0]!.icon" class="h-4 w-4 shrink-0" />
      {{ items[0]!.label }}
    </Button>

    <template v-else>
      <Button variant="outline" @click="toggle">
        <slot>
          {{ $t('dialogs.actions') }}
          <ChevronDown class="h-4 w-4" />
        </slot>
      </Button>

      <div
        v-if="open"
        class="absolute right-0 top-full mt-1 z-50 min-w-40 rounded-lg border border-border bg-card shadow-lg py-1"
      >
        <button
          v-for="item in items"
          :key="item.label"
          class="w-full flex items-center gap-2 px-3 py-2 text-sm text-left hover:bg-muted transition-colors duration-100 cursor-pointer"
          :class="item.danger ? 'text-destructive' : 'text-foreground'"
          @click="select(item)"
        >
          <component :is="item.icon" v-if="item.icon" class="h-4 w-4 shrink-0" />
          {{ item.label }}
        </button>
      </div>
    </template>
  </div>
</template>
