import { reactive, ref, computed } from 'vue'

export function useCardSelection(opts: { openGame: (id: number) => void }) {
  const selectMode = ref(false)
  const selectedGameIds = reactive(new Set<number>())
  const selectedCount = computed(() => selectedGameIds.size)

  function clearSelection() {
    selectedGameIds.clear()
  }

  function toggleSelectMode() {
    selectMode.value = !selectMode.value
    if (!selectMode.value) clearSelection()
  }

  function toggleGame(id: number) {
    if (selectedGameIds.has(id)) selectedGameIds.delete(id)
    else selectedGameIds.add(id)
  }

  const LONG_PRESS_MS = 450
  let pressTimer: number | null = null
  const pressFired = ref(false)
  let pressStartedAt = 0
  let autoEnteredSelectMode = false

  function cancelPress() {
    if (pressTimer !== null) {
      clearTimeout(pressTimer)
      pressTimer = null
    }
  }

  function beginPress(ev: Event | undefined, onHold: () => void) {
    cancelPress()
    pressFired.value = false
    autoEnteredSelectMode = false
    pressStartedAt = ev?.timeStamp ?? performance.now()
    pressTimer = window.setTimeout(() => {
      pressFired.value = true
      if (!selectMode.value) {
        selectMode.value = true
        autoEnteredSelectMode = true
      }
      onHold()
    }, LONG_PRESS_MS)
  }

  function wasRealHold(ev: Event | undefined): boolean {
    const releasedAt = ev?.timeStamp ?? performance.now()
    return releasedAt - pressStartedAt >= LONG_PRESS_MS
  }

  function undoAccidentalHold(revert: () => void) {
    pressFired.value = false
    revert()
    if (autoEnteredSelectMode && selectedCount.value === 0) selectMode.value = false
    autoEnteredSelectMode = false
  }

  function startPressGame(id: number, ev?: Event) {
    beginPress(ev, () => toggleGame(id))
  }

  function handleGameClick(id: number, ev?: Event) {
    cancelPress()
    if (pressFired.value) {
      if (wasRealHold(ev)) { pressFired.value = false; return }
      undoAccidentalHold(() => toggleGame(id))
    }
    if (selectMode.value) toggleGame(id)
    else opts.openGame(id)
  }

  return {
    selectMode,
    selectedGameIds,
    selectedCount,
    clearSelection,
    toggleSelectMode,
    toggleGame,
    cancelPress,
    startPressGame,
    handleGameClick,
  }
}
