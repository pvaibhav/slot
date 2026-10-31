<script setup lang="ts">
import { confirmState, resolveConfirm } from '@/lib/confirm'
import Button from '@/components/ui/Button.vue'
import Modal from '@/components/ui/Modal.vue'
</script>

<template>
  <Modal
    v-if="confirmState.pending"
    :title="confirmState.pending.title"
    @close="resolveConfirm(false)"
  >
    <p v-if="confirmState.pending.body" class="text-sm text-foreground/80">{{ confirmState.pending.body }}</p>
    <template #footer>
      <Button variant="ghost" @click="resolveConfirm(false)">{{ confirmState.pending.cancelLabel ?? $t('common.cancel') }}</Button>
      <Button
        :variant="confirmState.pending.destructive ? 'destructive' : 'default'"
        @click="resolveConfirm(true)"
      >{{ confirmState.pending.confirmLabel ?? $t('common.confirm') }}</Button>
    </template>
  </Modal>
</template>
