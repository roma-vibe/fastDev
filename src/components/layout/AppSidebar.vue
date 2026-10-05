<script setup lang="ts">
import { FolderKanban, LibraryBig, Plug, Settings } from '@lucide/vue'
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import UiStatusDot from '@/components/ui/UiStatusDot.vue'
import { t } from '@/i18n'
import { useAppStore } from '@/stores/app'
import { useLibraryStore } from '@/stores/library'
import { useProjectsStore } from '@/stores/projects'
import AppLogo from './AppLogo.vue'

const route = useRoute()
const library = useLibraryStore()
const projects = useProjectsStore()
const app = useAppStore()

const nav = computed(() => [
  {
    to: '/library',
    label: t('Library'),
    icon: LibraryBig,
    count: library.skeletons.length,
    match: '/library',
  },
  {
    to: '/projects',
    label: t('Projects'),
    icon: FolderKanban,
    count: projects.projects.length,
    match: '/projects',
  },
])

const recent = computed(() =>
  [...projects.projects]
    .sort((a, b) => (b.openedAt ?? b.createdAt).localeCompare(a.openedAt ?? a.createdAt))
    .slice(0, 6),
)
const mcpReady = computed(() => !!app.shellInfo?.controlSocket)
</script>

<template>
  <aside class="flex w-[228px] shrink-0 flex-col px-3 pt-[52px] pb-3" data-tauri-drag-region>
    <div class="mb-4 flex items-center gap-2.5 px-2.5" data-tauri-drag-region>
      <AppLogo :size="26" />
      <span class="text-[15px] font-semibold tracking-tight">fastDev</span>
    </div>

    <nav class="flex flex-col gap-0.5">
      <RouterLink
        v-for="item in nav"
        :key="item.to"
        :to="item.to"
        class="focus-ring flex h-9 items-center gap-2.5 rounded-xl px-2.5 text-[13.5px] transition-colors"
        :class="
          route.path.startsWith(item.match)
            ? 'bg-accent-soft font-semibold text-accent-fg'
            : 'text-fg-2 hover:bg-fill hover:text-fg'
        "
      >
        <component :is="item.icon" :size="17" :stroke-width="1.9" />
        <span class="flex-1">{{ item.label }}</span>
        <span v-if="item.count" class="text-[11.5px] tabular-nums opacity-60">{{ item.count }}</span>
      </RouterLink>
    </nav>

    <div v-if="recent.length" class="mt-6 min-h-0 flex-1 overflow-y-auto">
      <p class="px-2.5 pb-1.5 text-[11px] font-semibold tracking-wider text-fg-3 uppercase">
        {{ t('Recent') }}
      </p>
      <RouterLink
        v-for="project in recent"
        :key="project.id"
        :to="`/projects/${project.id}`"
        class="focus-ring flex h-8 items-center gap-2.5 rounded-xl px-2.5 text-[13px] transition-colors"
        :class="
          route.path === `/projects/${project.id}`
            ? 'bg-fill-2 text-fg'
            : 'text-fg-2 hover:bg-fill hover:text-fg'
        "
      >
        <UiStatusDot
          :status="
            project.running.length
              ? 'running'
              : !project.exists || project.setupStatus === 'failed'
                ? 'warning'
                : 'idle'
          "
        />
        <span class="truncate">{{ project.name }}</span>
      </RouterLink>
    </div>
    <div v-else class="flex-1" data-tauri-drag-region />

    <div class="mt-3 flex flex-col gap-0.5">
      <RouterLink
        to="/settings"
        class="focus-ring flex h-9 items-center gap-2.5 rounded-xl px-2.5 text-[13.5px] transition-colors"
        :class="
          route.path.startsWith('/settings')
            ? 'bg-accent-soft font-semibold text-accent-fg'
            : 'text-fg-2 hover:bg-fill hover:text-fg'
        "
      >
        <Settings :size="17" :stroke-width="1.9" />
        <span class="flex-1">{{ t('Settings') }}</span>
      </RouterLink>
      <RouterLink
        to="/settings#mcp"
        class="flex items-center gap-2 px-2.5 pt-2 text-[11.5px] text-fg-3 hover:text-fg-2"
        :title="mcpReady ? t('AI agents can connect through MCP') : t('The MCP socket is not available')"
      >
        <Plug :size="13" :stroke-width="2" />
        <span>MCP</span>
        <UiStatusDot :status="mcpReady ? 'running' : 'warning'" class="scale-75" />
      </RouterLink>
    </div>
  </aside>
</template>
