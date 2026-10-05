<script setup>
/**
 * Renders Authentik's dh-consent flow (the ak-stage-consent challenge).
 */
import { computed, onMounted, ref } from 'vue'
import AuthShell from '../components/AuthShell.vue'
import { runFlow } from '../api'

const props = defineProps({
  flowSlug: { type: String, default: 'dh-consent' },
  query: { type: String, default: '' },
})

const emit = defineEmits(['done'])

const challenge = ref(null)
const loading = ref(true)
const submitting = ref(false)
const pageError = ref('')

const permissions = computed(() => challenge.value?.permissions ?? [])
const applicationName = computed(
  () => challenge.value?.header_text?.replace(/^You'?re about to sign into\s*/i, '') ?? 'Digital Hub',
)

function errorMessages() {
  const out = []
  for (const list of Object.values(challenge.value?.response_errors ?? {})) {
    for (const e of list ?? []) out.push(typeof e === 'string' ? e : e.string)
  }
  return out
}

function handleRedirect(ch) {
  if (ch.component === 'xak-flow-redirect' && ch.to) {
    emit('done', { location: ch.to })
    return true
  }
  return false
}

async function decide(allow) {
  submitting.value = true
  pageError.value = ''
  try {
    const next = await runFlow(props.flowSlug, '', {
      component: 'ak-stage-consent',
      token: challenge.value.token,
      allow,
    })
    challenge.value = next
    handleRedirect(next)
  } catch (e) {
    pageError.value = e.message
  } finally {
    submitting.value = false
  }
}

onMounted(async () => {
  try {
    const first = await runFlow(props.flowSlug, props.query, null)
    challenge.value = first
    handleRedirect(first)
  } catch (e) {
    pageError.value = e.message
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <AuthShell subtitle="Permintaan otorisasi">
    <div v-if="loading" class="py-8 text-center text-sm text-slate-500">Memuat…</div>

    <div v-else-if="pageError" class="rounded-lg border border-rose-200 bg-rose-50 p-4">
      <p class="text-sm font-medium text-rose-800">Gagal memuat persetujuan</p>
      <p class="mt-1 text-xs text-rose-700">{{ pageError }}</p>
    </div>

    <div v-else class="mt-2 space-y-5">
      <div>
        <h2 class="text-base font-semibold text-slate-900">
          {{ applicationName }} meminta izin akses
        </h2>
        <p class="mt-1 text-sm text-slate-600">
          Pilih izin yang ingin Anda berikan sebelum melanjutkan.
        </p>
      </div>

      <div class="rounded-lg border border-slate-200">
        <p class="border-b border-slate-200 bg-slate-50 px-3 py-2 text-xs font-medium uppercase tracking-wide text-slate-500">
          Izin yang diminta
        </p>
        <ul v-if="permissions.length" class="divide-y divide-slate-100">
          <li v-for="p in permissions" :key="p.id" class="px-3 py-2.5">
            <p class="text-sm font-medium text-slate-800">{{ p.name || p.id }}</p>
            <p class="mt-0.5 font-mono text-xs text-slate-500">{{ p.id }}</p>
          </li>
        </ul>
        <p v-else class="px-3 py-4 text-sm text-slate-500">
          Tidak ada izin tambahan yang diminta selain akses dasar.
        </p>
      </div>

      <ul v-if="errorMessages().length" class="space-y-1 rounded-lg bg-rose-50 p-3">
        <li v-for="(m, i) in errorMessages()" :key="i" class="text-xs text-rose-700">{{ m }}</li>
      </ul>

      <div class="flex gap-3">
        <button
          type="button"
          :disabled="submitting"
          @click="decide(false)"
          class="flex-1 rounded-lg border border-slate-300 px-4 py-2.5 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-60"
        >
          Tolak
        </button>
        <button
          type="button"
          :disabled="submitting"
          @click="decide(true)"
          class="flex-1 rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-indigo-700 disabled:opacity-60"
        >
          {{ submitting ? 'Memproses…' : 'Izinkan' }}
        </button>
      </div>
    </div>
  </AuthShell>
</template>