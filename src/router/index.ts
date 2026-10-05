import { createRouter, createWebHashHistory } from 'vue-router'

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/library' },
    { path: '/library', component: () => import('@/pages/LibraryPage.vue') },
    { path: '/library/:id', component: () => import('@/pages/SkeletonPage.vue'), props: true },
    { path: '/projects', component: () => import('@/pages/ProjectsPage.vue') },
    { path: '/projects/:id', component: () => import('@/pages/ProjectPage.vue'), props: true },
    { path: '/settings', component: () => import('@/pages/SettingsPage.vue') },
    { path: '/:pathMatch(.*)*', redirect: '/library' },
  ],
  scrollBehavior: () => ({ top: 0 }),
})
