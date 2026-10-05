<script setup lang="ts">
import { computed, ref } from 'vue'
import { api, type SkeletonDetails } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiField from '@/components/ui/UiField.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import UiTextarea from '@/components/ui/UiTextarea.vue'
import UiToggleRow from '@/components/ui/UiToggleRow.vue'
import { t } from '@/i18n'
import { useToastStore } from '@/stores/toasts'

const props = defineProps<{ details: SkeletonDetails }>()
const emit = defineEmits<{ close: []; published: [version: string] }>()
const toasts = useToastStore()

type Bump = 'patch' | 'minor' | 'major'
const bump = ref<Bump>('minor')
const changes = ref('')
const allowUnverified = ref(false)
const publishing = ref(false)

const latest = computed(() => props.details.summary.latest)
const verified = computed(() => props.details.draftVerified === true)
const nextVersion = computed(() => {
  if (!latest.value) return '1.0.0'
  const [major = 0, minor = 0, patch = 0] = latest.value.split('.').map(Number)
  if (bump.value === 'major') return `${major + 1}.0.0`
  if (bump.value === 'minor') return `${major}.${minor + 1}.0`
  return `${major}.${minor}.${patch + 1}`
})
const lines = computed(() =>
  changes.value
    .split('\n')
    .map((l) => l.replace(/^[-*]\s*/, '').trim())
    .filter(Boolean),
)

async function publish(): Promise<void> {
  publishing.value = true
  try {
    const result = await api.publishSkeleton({
      id: props.details.summary.id,
      bump: latest.value ? bump.value : undefined,
      changes: lines.value,
      allow_unverified: allowUnverified.value,
    })
    const details = [
      t('Tag {tag}', { tag: result.tag }),
      result.pushed ? t('pushed to origin') : '',
      result.registry ? t('registry updated') : '',
      ...result.warnings,
    ].filter(Boolean)
    toasts.success(
      t('Published {id} {version}', { id: result.id, version: result.version }),
      details.join(' · '),
    )
    emit('published', result.version)
  } catch (e) {
    toasts.error(e)
  } finally {
    publishing.value = false
  }
}
</script>

<template>
  <UiSheet
    :title="t('Publish version {version}', { version: nextVersion })"
    :subtitle="details.summary.name"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-5 pb-2">
      <UiField
        v-if="latest"
        :label="t('Version change')"
        :hint="t('patch: fixes and patch updates · minor: new optional things · major: breaking changes')"
      >
        <UiSegmented
          v-model="bump"
          :options="[
            { value: 'patch', label: t('Patch') },
            { value: 'minor', label: t('Minor') },
            { value: 'major', label: t('Major') },
          ]"
        />
      </UiField>
      <UiField :label="t('Changelog')" :hint="t('One change per line.')">
        <UiTextarea v-model="changes" :rows="5" :placeholder="t('Updated Vue to 3.6')" />
      </UiField>
      <div v-if="!verified" class="rounded-2xl bg-warning/10 p-1">
        <UiToggleRow
          v-model="allowUnverified"
          :label="t('Publish without verification')"
          :description="
            t('The current draft has not passed Verify. Use only when the toolchain is not installed here.')
          "
        />
      </div>
    </div>
    <template #footer>
      <UiButton variant="secondary" @click="emit('close')">{{ t('Cancel') }}</UiButton>
      <UiButton
        variant="primary"
        :loading="publishing"
        :disabled="lines.length === 0 || (!verified && !allowUnverified)"
        @click="publish"
      >
        {{ t('Publish') }}
      </UiButton>
    </template>
  </UiSheet>
</template>
