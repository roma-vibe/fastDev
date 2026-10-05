<script setup lang="ts">
import { FolderOpen, Monitor, Moon, RefreshCw, Sun, X } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import {
  api,
  shell,
  type EditorKind,
  type Language,
  type TerminalApp,
  type Theme,
  type ToolInfo,
} from '@/api'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiBadge from '@/components/ui/UiBadge.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCard from '@/components/ui/UiCard.vue'
import UiCopyField from '@/components/ui/UiCopyField.vue'
import UiField from '@/components/ui/UiField.vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiSection from '@/components/ui/UiSection.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiSelect from '@/components/ui/UiSelect.vue'
import UiStatusDot from '@/components/ui/UiStatusDot.vue'
import UiToggleRow from '@/components/ui/UiToggleRow.vue'
import { t } from '@/i18n'
import { useAppStore } from '@/stores/app'
import { useSettingsStore } from '@/stores/settings'
import { useToastStore } from '@/stores/toasts'

const store = useSettingsStore()
const app = useAppStore()
const toasts = useToastStore()
const route = useRoute()
const tools = ref<ToolInfo[]>([])
const toolPath = ref('')
const refreshing = ref(false)
const editorCommand = ref(store.settings?.editorCommand ?? '')

const settings = computed(() => store.settings)

async function update(patch: Parameters<typeof store.update>[0]): Promise<void> {
  try {
    await store.update(patch)
  } catch (e) {
    toasts.error(e)
  }
}

async function loadTools(refresh = false): Promise<void> {
  refreshing.value = true
  try {
    const result = await api.toolchain(refresh)
    tools.value = result.tools
    toolPath.value = result.path
  } finally {
    refreshing.value = false
  }
}

onMounted(async () => {
  await Promise.all([loadTools(), app.load()])
  if (route.hash === '#mcp') document.getElementById('mcp')?.scrollIntoView({ behavior: 'smooth' })
})

const newRegistry = ref('')

async function addRegistry(value?: string): Promise<void> {
  const url = (value ?? newRegistry.value).trim()
  if (!url || !settings.value) return
  if (!settings.value.registries.includes(url))
    await update({ registries: [...settings.value.registries, url] })
  newRegistry.value = ''
}

async function chooseRegistryFolder(): Promise<void> {
  const path = await shell.pickFolder()
  if (path) await addRegistry(path)
}

async function removeRegistry(url: string): Promise<void> {
  if (settings.value) await update({ registries: settings.value.registries.filter((r) => r !== url) })
}

async function chooseWorkspace(): Promise<void> {
  const path = await shell.pickFolder(settings.value?.workspacePath)
  if (path) await update({ workspacePath: path })
}

async function removeSource(url: string): Promise<void> {
  try {
    await api.removeSkeletonSource(url)
    await store.load()
  } catch (e) {
    toasts.error(e)
  }
}

async function chooseProjectsDir(): Promise<void> {
  const path = await shell.pickFolder(settings.value?.projectsDir)
  if (path) await update({ projectsDir: path })
}

const themes = computed(() => [
  { value: 'system' as Theme, label: t('System'), icon: Monitor },
  { value: 'light' as Theme, label: t('Light'), icon: Sun },
  { value: 'dark' as Theme, label: t('Dark'), icon: Moon },
])
const languages = computed(() => [
  { value: 'system' as Language, label: t('System') },
  { value: 'en' as Language, label: 'English' },
  { value: 'ru' as Language, label: 'Русский' },
])
const editors = computed(() => [
  { value: 'vscode', label: 'Visual Studio Code' },
  { value: 'cursor', label: 'Cursor' },
  { value: 'zed', label: 'Zed' },
  { value: 'custom', label: t('Custom command') },
])
const terminals = [
  { value: 'terminal', label: 'Terminal' },
  { value: 'iterm', label: 'iTerm' },
  { value: 'warp', label: 'Warp' },
  { value: 'ghostty', label: 'Ghostty' },
]
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader
      :title="t('Settings')"
      :subtitle="app.appInfo ? `fastDev ${app.appInfo.version}` : undefined"
    />

    <div v-if="settings" class="flex-1 overflow-y-auto px-7 pb-10">
      <div class="mx-auto flex max-w-[860px] flex-col gap-8">
        <UiSection :title="t('Appearance')">
          <UiCard class="flex flex-col gap-5">
            <UiField :label="t('Theme')">
              <UiSegmented
                :model-value="settings.theme"
                :options="themes"
                @update:model-value="update({ theme: $event })"
              />
            </UiField>
            <UiField
              :label="t('Language')"
              :hint="t('Documentation and files created by fastDev are always in English.')"
            >
              <UiSegmented
                :model-value="settings.language"
                :options="languages"
                @update:model-value="update({ language: $event })"
              />
            </UiField>
          </UiCard>
        </UiSection>

        <UiSection :title="t('Folders')">
          <UiCard class="flex flex-col gap-5">
            <UiField
              :label="t('Skeleton registries')"
              :hint="
                t('Git repositories or local folders with an index.json that lists skeleton repositories.')
              "
            >
              <div class="flex flex-col gap-1.5">
                <div
                  v-for="url in settings.registries"
                  :key="url"
                  class="flex items-center gap-2 rounded-xl bg-fill py-1 pr-1 pl-3"
                >
                  <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px]">{{ url }}</code>
                  <UiIconButton
                    :icon="X"
                    :label="t('Remove')"
                    size="sm"
                    variant="ghost"
                    @click="removeRegistry(url)"
                  />
                </div>
                <div class="flex gap-2">
                  <UiInput
                    v-model="newRegistry"
                    mono
                    class="flex-1"
                    placeholder="https://github.com/owner/fastdev-registry.git"
                    @keydown.enter="addRegistry()"
                  />
                  <UiButton :disabled="!newRegistry.trim()" @click="addRegistry()">{{ t('Add') }}</UiButton>
                  <UiButton :icon="FolderOpen" @click="chooseRegistryFolder">{{ t('Folder…') }}</UiButton>
                </div>
              </div>
            </UiField>
            <UiField
              :label="t('Workspace')"
              :hint="t('Git repositories of the skeletons you and your agents are authoring.')"
            >
              <div class="flex gap-2">
                <code
                  class="selectable flex h-9 min-w-0 flex-1 items-center truncate rounded-xl bg-fill px-3 font-mono text-[12px]"
                >
                  {{ settings.workspacePath }}
                </code>
                <UiButton :icon="FolderOpen" @click="chooseWorkspace">{{ t('Choose…') }}</UiButton>
              </div>
            </UiField>
            <UiField v-if="settings.skeletonSources.length" :label="t('Skeletons added by URL')">
              <div class="flex flex-col gap-1.5">
                <div
                  v-for="url in settings.skeletonSources"
                  :key="url"
                  class="flex items-center gap-2 rounded-xl bg-fill py-1 pr-1 pl-3"
                >
                  <code class="selectable min-w-0 flex-1 truncate font-mono text-[12px]">{{ url }}</code>
                  <UiIconButton
                    :icon="X"
                    :label="t('Remove')"
                    size="sm"
                    variant="ghost"
                    @click="removeSource(url)"
                  />
                </div>
              </div>
            </UiField>
            <UiField :label="t('Projects folder')" :hint="t('Where new projects are created by default.')">
              <div class="flex gap-2">
                <code
                  class="selectable flex h-9 min-w-0 flex-1 items-center truncate rounded-xl bg-fill px-3 font-mono text-[12px]"
                >
                  {{ settings.projectsDir }}
                </code>
                <UiButton :icon="FolderOpen" @click="chooseProjectsDir">{{ t('Choose…') }}</UiButton>
              </div>
            </UiField>
          </UiCard>
        </UiSection>

        <UiSection :title="t('Tools')">
          <UiCard class="flex flex-col gap-5">
            <div class="flex flex-wrap gap-8">
              <UiField :label="t('Editor')">
                <UiSelect
                  :model-value="settings.editor"
                  :options="editors"
                  @update:model-value="update({ editor: $event as EditorKind })"
                />
              </UiField>
              <UiField :label="t('Terminal')">
                <UiSelect
                  :model-value="settings.terminal"
                  :options="terminals"
                  @update:model-value="update({ terminal: $event as TerminalApp })"
                />
              </UiField>
            </div>
            <UiField
              v-if="settings.editor === 'custom'"
              :label="t('Editor command')"
              :hint="t('{path} is replaced with the project folder, e.g. idea {path}')"
            >
              <UiInput
                v-model="editorCommand"
                mono
                placeholder="code {path}"
                @change="update({ editorCommand })"
              />
            </UiField>
          </UiCard>
        </UiSection>

        <UiSection :title="t('Toolchain')" :description="t('Found with the PATH of your login shell.')">
          <template #actions>
            <UiButton
              size="sm"
              variant="ghost"
              :icon="RefreshCw"
              :loading="refreshing"
              @click="loadTools(true)"
            >
              {{ t('Refresh') }}
            </UiButton>
          </template>
          <UiCard padding="none" class="grid grid-cols-1 divide-hairline sm:grid-cols-2">
            <div
              v-for="tool in tools"
              :key="tool.name"
              class="flex items-center gap-3 border-b border-hairline px-5 py-2.5"
            >
              <UiStatusDot :status="tool.found ? 'running' : 'idle'" class="[&>span:first-child]:hidden" />
              <span class="w-20 text-[13px] font-medium">{{ tool.name }}</span>
              <span class="font-mono text-[12px]" :class="tool.found ? 'text-fg-2' : 'text-fg-3'">
                {{ tool.found ? tool.version : t('not installed') }}
              </span>
              <span class="ml-auto truncate font-mono text-[11px] text-fg-3" :title="tool.path ?? ''">{{
                tool.path
              }}</span>
            </div>
          </UiCard>
        </UiSection>

        <UiSection :title="t('Behaviour')">
          <UiCard padding="sm" class="flex flex-col">
            <UiToggleRow
              :model-value="settings.keepInMenuBar"
              :label="t('Keep running in the menu bar')"
              :description="t('Closing the window hides it; running projects and MCP keep working.')"
              @update:model-value="update({ keepInMenuBar: $event })"
            />
            <UiToggleRow
              :model-value="settings.pushOnPublish"
              :label="t('Push on publish')"
              :description="
                t('Push the skeleton repository, its new tag and the registry to origin when publishing.')
              "
              @update:model-value="update({ pushOnPublish: $event })"
            />
          </UiCard>
        </UiSection>

        <UiSection
          id="mcp"
          :title="t('MCP for AI agents')"
          :description="
            t('Agents create projects and maintain skeletons through fastDev only while the app is running.')
          "
        >
          <UiCard class="flex flex-col gap-5">
            <div class="flex items-center gap-2 text-[13px]">
              <UiStatusDot :status="app.shellInfo?.controlSocket ? 'running' : 'warning'" />
              <span v-if="app.shellInfo?.controlSocket">{{ t('Listening for agents') }}</span>
              <span v-else>{{ app.shellInfo?.controlError ?? t('The MCP socket is not available') }}</span>
              <UiBadge v-if="app.appInfo && !app.appInfo.bridgeExists" tone="warning" class="ml-auto">
                {{ t('Bridge binary not built yet') }}
              </UiBadge>
            </div>
            <UiField
              :label="t('Claude Code')"
              :hint="t('Run once in a terminal; the server is then available in every Claude Code session.')"
            >
              <UiCopyField :value="app.appInfo?.claudeCommand ?? ''" />
            </UiField>
            <UiField :label="t('Codex')" :hint="t('Add to ~/.codex/config.toml')">
              <UiCopyField :value="app.appInfo?.codexConfig ?? ''" multiline />
            </UiField>
          </UiCard>
        </UiSection>

        <p class="text-center text-[12px] text-fg-3">
          {{ t('Data folder') }}: <span class="selectable font-mono">{{ app.appInfo?.dataDir }}</span>
        </p>
      </div>
    </div>
  </div>
</template>
