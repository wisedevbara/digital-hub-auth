<script setup>
/**
 * Shown when the identity provider denies the request (HTTP 403 / access_denied).
 */
import { computed } from 'vue'
import AuthShell from '../components/AuthShell.vue'

const props = defineProps({
  error: { type: String, default: '' },
  errorDescription: { type: String, default: '' },
  brandName: { type: String, default: 'Digital Hub' },
  clientName: { type: String, default: '' },
})

// The parent decides where "Kembali" goes (the configured start URL for this
// client), so this view stays free of any app-specific route.
const emit = defineEmits(['retry'])

const heading = computed(() =>
  props.error === 'access_denied'
    ? 'Akses ditolak'
    : 'Tidak diizinkan',
)
</script>

<template>
  <AuthShell :brand-name="props.brandName" :client-name="props.clientName">
    <div class="flex flex-col items-center text-center">
      <div
        class="flex h-12 w-12 items-center justify-center rounded-full bg-rose-50 text-rose-600 ring-1 ring-inset ring-rose-600/20"
      >
        <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round"
                d="M18.364 18.364A9 9 0 0 0 5.636 5.636m12.728 12.728A9 9 0 0 1 5.636 5.636m12.728 12.728L5.636 5.636" />
        </svg>
      </div>

      <h2 class="mt-5 text-lg font-semibold text-slate-900">{{ heading }}</h2>
      <p class="mt-2 text-sm text-slate-600">
        {{ errorDescription || 'Anda tidak memiliki izin untuk mengakses aplikasi ini.' }}
      </p>

      <p v-if="error" class="mt-4 rounded-lg bg-slate-50 px-3 py-2 font-mono text-xs text-slate-500">
        error: {{ error }}
      </p>

      <button
        type="button"
        @click="emit('retry')"
        class="mt-6 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-indigo-700"
      >
        Kembali
      </button>
    </div>
  </AuthShell>
</template>