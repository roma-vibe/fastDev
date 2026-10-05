<script setup lang="ts">
import { FolderOpen, LibraryBig, Plus, RefreshCw, Search, Star } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { api } from '@/api'
import SkeletonCard from '@/components/library/SkeletonCard.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiField from '@/components/ui/UiField.vue'
import UiSheet from '@/components/ui/UiSheet.vue'
import UiEmpty from '@/components/ui/UiEmpty.vue'
import UiIconButton from '@/components/ui/UiIconButton.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiPill from '@/components/ui/UiPill.vue'
import { st, t, tn } from '@/i18n'
import { language } from '@/lib/languages'
import { useLibraryStore } from '@/stores/library'
import { useToastStore } from '@/stores/toasts'

const library = useLibraryStore()
const toasts = useToastStore()
const query = ref('')
const lang = ref('all')
const category = ref('all')
const favoritesOnly = ref(false)

onMounted(() => void library.load())

const languages = computed(() => [...new Set(library.skeletons.flatMap((s) => s.languages))].sort())
const categories = computed(() => [...new Set(library.skeletons.map((s) => s.category))].sort())

function categoryName(id: string): string {
  switch (id) {
    case 'web':
      return t('Web')
    case 'desktop':
      return t('Desktop')
    case 'cli':
      return t('CLI')
    case 'library':
      return t('Libraries')
    case 'mobile':
      return t('Mobile')
    default:
      return t('Other')
  }
}

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  return library.skeletons
    .filter((s) => lang.value === 'all' || s.languages.includes(lang.value))
    .filter((s) => category.value === 'all' || s.category === category.value)
    .filter((s) => !favoritesOnly.value || s.favorite)
    .filter(
      (s) =>
        !q ||
        [
          s.id,
          s.name,
          s.description,
          st(s.name, s.translations),
          st(s.description, s.translations),
          ...s.tags,
          ...s.stack,
        ].some((field) => field.toLowerCase().includes(q)),
    )
    .sort((a, b) => Number(!!b.favorite) - Number(!!a.favorite) || a.name.localeCompare(b.name))
})

async function favorite(id: string, value: boolean): Promise<void> {
  try {
    await library.toggleFavorite(id, value)
  } catch (e) {
    toasts.error(e)
  }
}

const syncing = ref(false)
async function sync(): Promise<void> {
  syncing.value = true
  try {
    const report = await api.syncLibrary()
    const failed = [...report.registries, ...report.skeletons].filter((item) => !item.ok)
    if (failed.length) {
      toasts.push(
        'error',
        t('Some sources could not be updated'),
        failed.map((f) => `${f.name}: ${f.error}`).join('\n'),
      )
    } else {
      toasts.success(t('Library is up to date'))
    }
    await library.load()
  } catch (e) {
    toasts.error(e)
  } finally {
    syncing.value = false
  }
}

const addOpen = ref(false)
const addUrl = ref('')
const adding = ref(false)
async function addSource(): Promise<void> {
  adding.value = true
  try {
    const result = await api.addSkeletonSource(addUrl.value.trim())
    toasts.success(t('Skeleton added'), result.id)
    addOpen.value = false
    addUrl.value = ''
    await library.load()
  } catch (e) {
    toasts.error(e)
  } finally {
    adding.value = false
  }
}

async function openFolder(): Promise<void> {
  if (!library.workspacePath) return
  try {
    await api.openPath(library.workspacePath)
  } catch (e) {
    toasts.error(e)
  }
}
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader
      :title="t('Library')"
      :subtitle="library.error ? undefined : tn(library.skeletons.length, '{n} skeleton', '{n} skeletons')"
    >
      <UiInput v-model="query" :icon="Search" :placeholder="t('Search skeletons')" class="w-60" />
      <UiIconButton
        :icon="Star"
        :label="t('Favorites only')"
        :active="favoritesOnly"
        @click="favoritesOnly = !favoritesOnly"
      />
      <UiIconButton :icon="RefreshCw" :label="t('Update library')" :disabled="syncing" @click="sync" />
      <UiIconButton :icon="Plus" :label="t('Add skeleton by URL')" @click="addOpen = true" />
      <UiIconButton :icon="FolderOpen" :label="t('Open workspace folder')" @click="openFolder" />
    </PageHeader>

    <div class="flex-1 overflow-y-auto px-7 pb-8">
      <UiEmpty
        v-if="library.error"
        :icon="LibraryBig"
        :title="t('The library folder is not available')"
        :text="library.error"
      >
        <RouterLink to="/settings" custom>
          <template #default="{ navigate }">
            <UiButton variant="primary" @click="navigate">{{ t('Choose library folder') }}</UiButton>
          </template>
        </RouterLink>
      </UiEmpty>

      <template v-else>
        <div class="flex flex-wrap items-center gap-2 pb-5">
          <UiPill :active="lang === 'all'" @click="lang = 'all'">{{ t('All') }}</UiPill>
          <UiPill v-for="id in languages" :key="id" :active="lang === id" @click="lang = id">
            {{ language(id).name }}
          </UiPill>
          <span v-if="categories.length > 1" class="mx-1 h-5 w-px bg-hairline" />
          <template v-if="categories.length > 1">
            <UiPill
              v-for="id in categories"
              :key="id"
              :active="category === id"
              @click="category = category === id ? 'all' : id"
            >
              {{ categoryName(id) }}
            </UiPill>
          </template>
        </div>

        <UiEmpty
          v-if="library.loaded && filtered.length === 0"
          :icon="Search"
          :title="library.skeletons.length ? t('Nothing matches the filters') : t('The library is empty')"
          :text="
            library.skeletons.length
              ? t('Try another search or filter.')
              : library.registries.length
                ? t('Ask your AI agent to create a skeleton through the fastDev MCP server.')
                : t('No skeleton registry is configured. Add one in Settings or add a skeleton by URL.')
          "
        />

        <div class="grid grid-cols-[repeat(auto-fill,minmax(270px,1fr))] gap-4">
          <SkeletonCard
            v-for="skeleton in filtered"
            :key="skeleton.id"
            :skeleton="skeleton"
            :favorite="!!skeleton.favorite"
            @favorite="favorite(skeleton.id, $event)"
          />
        </div>
      </template>
    </div>

    <UiSheet
      v-if="addOpen"
      :title="t('Add skeleton by URL')"
      :subtitle="t('A git repository of a fastDev skeleton with vX.Y.Z tags.')"
      width="sm"
      @close="addOpen = false"
    >
      <div class="pb-2">
        <UiField :label="t('Repository')" :hint="t('https://github.com/owner/repo.git or a local folder')">
          <UiInput
            v-model="addUrl"
            mono
            placeholder="https://github.com/owner/fastdev-node-vue.git"
            autofocus
          />
        </UiField>
      </div>
      <template #footer>
        <UiButton variant="secondary" @click="addOpen = false">{{ t('Cancel') }}</UiButton>
        <UiButton variant="primary" :loading="adding" :disabled="!addUrl.trim()" @click="addSource">
          {{ t('Add') }}
        </UiButton>
      </template>
    </UiSheet>
  </div>
</template>
