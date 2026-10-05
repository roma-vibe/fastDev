<script setup lang="ts">
import { Bot, Eye, FileText, FolderOpen, GitBranch, Package, Rocket, Sparkles } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { api, shell, type ResolvedRequirements, type SkeletonDetails } from '@/api'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiChoiceGroup from '@/components/ui/UiChoiceGroup.vue'
import UiField from '@/components/ui/UiField.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiTextarea from '@/components/ui/UiTextarea.vue'
import UiToggleRow from '@/components/ui/UiToggleRow.vue'
import { st, t } from '@/i18n'
import { isValidSlug, slugify } from '@/lib/slug'
import { useJobsStore } from '@/stores/jobs'
import { useSettingsStore } from '@/stores/settings'
import { useToastStore } from '@/stores/toasts'
import { useRunsStore } from '@/stores/runs'
import MissingTools from './MissingTools.vue'
import PreviewSheet from './PreviewSheet.vue'
import RequirementList from './RequirementList.vue'

const props = defineProps<{ details: SkeletonDetails }>()

const settings = useSettingsStore()
const jobs = useJobsStore()
const toasts = useToastStore()

const name = ref('')
const slug = ref('')
const slugEdited = ref(false)
const parent = ref(settings.settings?.projectsDir ?? '')
const git = ref(true)
const install = ref(true)
const agentsMd = ref(true)
const specMd = ref(true)
const claudeMd = ref(true)
const brief = ref('')
const features = ref<Record<string, boolean>>({})
const choices = ref<Record<string, string>>({})
const resolved = ref<ResolvedRequirements | null>(null)
const creating = ref(false)
const previewOpen = ref(false)
const previewAttach = ref(false)
const runs = useRunsStore()
const previewRunning = computed(() =>
  Object.values(runs.runs).some(
    (r) => r.projectId === `preview:${props.details.summary.id}` && r.status === 'running',
  ),
)

function openPreview(): void {
  previewAttach.value = previewRunning.value
  previewOpen.value = true
}

const manifest = computed(() => props.details.manifest)
watch(
  manifest,
  (m) => {
    features.value = Object.fromEntries(Object.entries(m?.features ?? {}).map(([k, f]) => [k, f.default]))
    choices.value = Object.fromEntries(Object.entries(m?.choices ?? {}).map(([k, c]) => [k, c.default]))
  },
  { immediate: true },
)

/** Choices whose `when` condition is met by the current selection (same rule as the core). */
const visibleChoices = computed(() =>
  Object.entries(manifest.value?.choices ?? {}).filter(([, choice]) =>
    Object.entries(choice.when ?? {}).every(([other, allowed]) => {
      const value = choices.value[other] ?? ''
      return Array.isArray(allowed) ? allowed.includes(value) : allowed === value
    }),
  ),
)

// Requirements and setup depend on the selection; the core resolves them.
let requestId = 0
watch(
  [features, choices, () => props.details.target],
  async () => {
    const id = ++requestId
    try {
      const result = await api.skeletonRequirements(
        props.details.summary.id,
        props.details.target,
        features.value,
        choices.value,
      )
      if (id === requestId) resolved.value = result
    } catch {
      if (id === requestId) resolved.value = null
    }
  },
  { deep: true, immediate: true },
)
watch(name, (value) => {
  if (!slugEdited.value) slug.value = slugify(value)
})
watch(agentsMd, (value) => {
  if (!value) claudeMd.value = false
})

const isDraft = computed(() => props.details.target === 'draft')
const targetPath = computed(() =>
  parent.value && slug.value ? `${parent.value.replace(/\/$/, '')}/${slug.value}` : '',
)
const slugError = computed(() =>
  slug.value && !isValidSlug(slug.value)
    ? t('Use lowercase latin letters, digits, dashes, dots or underscores')
    : null,
)
const requirements = computed(() => resolved.value?.requirements ?? props.details.requirements)
/** Missing tools that setup needs; other missing tools are only warnings. */
const missing = computed(() => requirements.value.filter((r) => !r.satisfied && r.neededForSetup !== false))
const setupText = computed(() =>
  (resolved.value?.setup ?? manifest.value?.setup ?? []).map((s) => s.run).join(' · '),
)
const canCreate = computed(
  () =>
    !isDraft.value &&
    name.value.trim().length > 0 &&
    !!slug.value &&
    !slugError.value &&
    !!parent.value &&
    !(install.value && missing.value.length > 0),
)

async function chooseFolder(): Promise<void> {
  const folder = await shell.pickFolder(parent.value || undefined)
  if (folder) parent.value = folder
}

async function create(): Promise<void> {
  creating.value = true
  try {
    const response = await api.createProject({
      skeleton: props.details.summary.id,
      version: props.details.target,
      name: name.value.trim(),
      slug: slug.value,
      parent_dir: parent.value,
      git: git.value,
      install: install.value,
      features: features.value,
      choices: choices.value,
      agents_md: agentsMd.value,
      spec_md: specMd.value,
      claude_md: claudeMd.value,
      brief: brief.value,
    })
    await jobs.track(response, {
      onSuccess: () => {
        name.value = ''
        brief.value = ''
        slugEdited.value = false
      },
    })
  } catch (e) {
    toasts.error(e)
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <div v-if="isDraft" class="rounded-card bg-warning/10 px-5 py-4 text-[13px] leading-relaxed text-warning">
    {{ t('This is a draft. Publish it on the Maintenance tab to create projects from it.') }}
  </div>

  <div v-else class="grid grid-cols-1 gap-5 @3xl:grid-cols-[minmax(0,1.35fr)_minmax(300px,1fr)]">
    <UiCard class="flex flex-col gap-5">
      <UiField
        :label="t('Project name')"
        :hint="t('Any language. It goes into APP_NAME in .env and .env.example.')"
      >
        <UiInput v-model="name" :placeholder="t('My new project')" autofocus />
      </UiField>

      <UiField :label="t('Slug')" :hint="t('Folder and package name.')" :error="slugError">
        <UiInput v-model="slug" mono placeholder="my-new-project" @input="slugEdited = true" />
      </UiField>

      <UiField
        :label="t('Location')"
        :hint="targetPath ? t('The project will be created in {path}', { path: targetPath }) : undefined"
      >
        <div class="flex gap-2">
          <UiInput v-model="parent" mono class="flex-1" />
          <UiButton :icon="FolderOpen" @click="chooseFolder">{{ t('Choose…') }}</UiButton>
        </div>
      </UiField>

      <UiField
        :label="t('Initial prompt')"
        :hint="t('The idea of the project for your AI agent. It is written into SPEC.md → Initial brief.')"
      >
        <UiTextarea
          v-model="brief"
          :rows="7"
          :placeholder="t('Describe what you want to build: users, main features, constraints…')"
        />
      </UiField>
    </UiCard>

    <div class="flex flex-col gap-5">
      <UiCard v-if="visibleChoices.length" padding="sm" class="flex flex-col gap-3">
        <div v-for="[key, choice] in visibleChoices" :key="key" class="flex flex-col gap-1">
          <p class="px-3 pt-1 text-[12px] font-semibold text-fg-2">
            {{ st(choice.label, manifest?.translations) || key }}
          </p>
          <UiChoiceGroup
            :name="st(choice.label, manifest?.translations) || key"
            :model-value="choices[key] ?? choice.default"
            :options="
              Object.entries(choice.options).map(([value, option]) => ({
                value,
                label: st(option.label, manifest?.translations) || value,
                description: st(option.description, manifest?.translations) || undefined,
              }))
            "
            @update:model-value="choices[key] = $event"
          />
        </div>
      </UiCard>

      <UiCard padding="sm" class="flex flex-col">
        <UiToggleRow
          v-model="git"
          :icon="GitBranch"
          :label="t('Git repository')"
          :description="t('git init and the first commit')"
        />
        <UiToggleRow
          v-model="install"
          :icon="Package"
          :label="t('Install dependencies')"
          :description="setupText || t('No setup steps')"
        />
        <UiToggleRow
          v-for="(feature, key) in manifest?.features ?? {}"
          :key="key"
          :model-value="features[key] ?? false"
          :icon="Sparkles"
          :label="st(feature.label, manifest?.translations) || key"
          :description="st(feature.description, manifest?.translations)"
          @update:model-value="features[key] = $event"
        />
        <div class="mx-3 my-1.5 h-px bg-hairline" />
        <UiToggleRow
          v-model="agentsMd"
          :icon="Bot"
          label="AGENTS.md"
          :description="t('How agents work with this project')"
        />
        <UiToggleRow
          v-model="specMd"
          :icon="FileText"
          label="SPEC.md"
          :description="t('Project specification with your prompt')"
        />
        <UiToggleRow
          v-model="claudeMd"
          :icon="Bot"
          label="CLAUDE.md"
          :description="t('Makes Claude Code read AGENTS.md')"
          :disabled="!agentsMd"
        />
      </UiCard>

      <UiCard padding="sm" class="flex flex-col gap-2 px-4">
        <p class="text-[12px] font-semibold text-fg-2">{{ t('Requirements') }}</p>
        <RequirementList v-if="requirements.length" :items="requirements" />
        <p v-else class="text-[12.5px] text-fg-3">{{ t('No requirements') }}</p>
      </UiCard>

      <MissingTools :items="requirements" :install="install" />

      <div class="flex flex-col gap-2">
        <UiButton
          variant="primary"
          size="lg"
          :icon="Rocket"
          :loading="creating"
          :disabled="!canCreate"
          @click="create"
        >
          {{ t('Create project') }}
        </UiButton>
        <UiButton size="lg" :icon="Eye" @click="openPreview">
          {{ previewRunning ? t('Preview is running — open') : t('Preview without creating') }}
        </UiButton>
      </div>
      <PreviewSheet
        v-if="previewOpen"
        :details="details"
        :features="features"
        :choices="choices"
        :attach="previewAttach"
        @close="previewOpen = false"
      />
    </div>
  </div>
</template>
