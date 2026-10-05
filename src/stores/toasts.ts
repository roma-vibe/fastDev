import { defineStore } from 'pinia'
import { ref } from 'vue'
import { ApiError } from '@/api'
import { t } from '@/i18n'

export type ToastTone = 'info' | 'success' | 'error'

export interface Toast {
  id: number
  tone: ToastTone
  title: string
  detail?: string
}

function errorTitle(code: string): string {
  switch (code) {
    case 'not_found':
      return t('Not found')
    case 'invalid_input':
      return t('Invalid input')
    case 'conflict':
      return t('Conflict')
    case 'validation':
      return t('Validation failed')
    case 'toolchain':
      return t('Missing tools')
    case 'command_failed':
      return t('Command failed')
    case 'not_allowed':
      return t('Not allowed')
    case 'io':
      return t('File system error')
    default:
      return t('Something went wrong')
  }
}

export const useToastStore = defineStore('toasts', () => {
  const toasts = ref<Toast[]>([])
  let next = 1

  function push(tone: ToastTone, title: string, detail?: string): void {
    const toast: Toast = { id: next++, tone, title, detail }
    toasts.value = [...toasts.value, toast]
    setTimeout(() => dismiss(toast.id), tone === 'error' ? 8000 : 3500)
  }

  function dismiss(id: number): void {
    toasts.value = toasts.value.filter((t) => t.id !== id)
  }

  function error(e: unknown): void {
    if (e instanceof ApiError) {
      push('error', errorTitle(e.code), e.message)
    } else {
      push('error', t('Something went wrong'), e instanceof Error ? e.message : String(e))
    }
  }

  return {
    toasts,
    push,
    dismiss,
    error,
    success: (title: string, detail?: string) => push('success', title, detail),
  }
})
