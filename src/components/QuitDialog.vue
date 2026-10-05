<script setup lang="ts">
import { ref } from 'vue'
import { shell } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import { t, tn } from '@/i18n'
import { useAppStore } from '@/stores/app'

const app = useAppStore()
const quitting = ref(false)

async function quit(): Promise<void> {
  quitting.value = true
  await shell.quit()
}
</script>

<template>
  <UiSheet
    v-if="app.quitRequest !== null"
    :title="t('Quit fastDev?')"
    width="sm"
    @close="app.quitRequest = null"
  >
    <p class="pb-2 text-[13.5px] leading-relaxed text-fg-2">
      {{
        tn(
          app.quitRequest,
          '{n} project process is running. It will be stopped.',
          '{n} project processes are running. They will be stopped.',
        )
      }}
    </p>
    <template #footer>
      <UiButton variant="secondary" @click="app.quitRequest = null">{{ t('Cancel') }}</UiButton>
      <UiButton variant="danger" :loading="quitting" @click="quit">{{ t('Stop and quit') }}</UiButton>
    </template>
  </UiSheet>
</template>
