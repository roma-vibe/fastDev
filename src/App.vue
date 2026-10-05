<script setup lang="ts">
import JobSheet from '@/components/JobSheet.vue'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import ToastHost from '@/components/layout/ToastHost.vue'
import QuitDialog from '@/components/QuitDialog.vue'
</script>

<template>
  <div class="flex h-full">
    <AppSidebar />
    <main
      class="my-2 mr-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-[22px] border border-hairline bg-content shadow-panel"
    >
      <RouterView v-slot="{ Component, route }">
        <Transition name="page" mode="out-in">
          <component :is="Component" :key="route.path" />
        </Transition>
      </RouterView>
    </main>
    <JobSheet />
    <QuitDialog />
    <ToastHost />
  </div>
</template>

<style>
.page-enter-active,
.page-leave-active {
  transition:
    opacity 140ms ease,
    transform 180ms ease;
}
.page-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.page-leave-to {
  opacity: 0;
}
</style>
