<script setup>
/**
 * Shown for OIDC protocol errors and unexpected failures, so the user never
 * lands on a blank page or an Authentik-styled error.
 */
import AuthShell from '../components/AuthShell.vue'

const props = defineProps({
  error: { type: String, default: 'server_error' },
  errorDescription: { type: String, default: '' },
  detail: { type: String, default: '' },
  brandName: { type: String, default: 'Digital Hub' },
  clientName: { type: String, default: '' },
})

// The parent resolves the retry target from /config, so a second client app does
// not need its own hardcoded link in this view.
const emit = defineEmits(['retry'])
</script>

<template>
  <AuthShell :brand-name="props.brandName" :client-name="props.clientName">
    <div class="flex flex-col items-center text-center">
      <div
        class="flex h-12 w-12 items-center justify-center rounded-full bg-amber-50 text-amber-600 ring-1 ring-inset ring-amber-600/20"
      >
        <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round"
                d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z" />
        </svg>
      </div>

      <h2 class="mt-5 text-lg font-semibold text-slate-900">Terjadi kesalahan OIDC</h2>
      <p class="mt-2 text-sm text-slate-600">
        {{
          errorDescription ||
          'Permintaan autentikasi tidak dapat diproses. Silakan coba lagi atau hubungi administrator.'
        }}
      </p>

      <div class="mt-4 w-full space-y-2 text-left">
        <div class="rounded-lg bg-slate-50 px-3 py-2">
          <p class="text-xs font-medium uppercase tracking-wide text-slate-500">Kode error</p>
          <p class="mt-0.5 font-mono text-xs text-slate-700">{{ error }}</p>
        </div>
        <div v-if="detail" class="rounded-lg bg-slate-50 px-3 py-2">
          <p class="text-xs font-medium uppercase tracking-wide text-slate-500">Detail</p>
          <p class="mt-0.5 break-all font-mono text-xs text-slate-700">{{ detail }}</p>
        </div>
      </div>

      <button
        type="button"
        @click="emit('retry')"
        class="mt-6 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-indigo-700"
      >
        Coba lagi
      </button>
    </div>
  </AuthShell>
</template>