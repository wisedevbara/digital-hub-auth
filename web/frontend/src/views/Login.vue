<script setup>
/**
 * Drives Authentik's dh-login flow from the challenge responses.
 *
 * Component names come straight from the flow executor API:
 *   ak-stage-identification -> asks for the identifier
 *   ak-stage-password       -> asks for the password
 *   xak-flow-redirect       -> finished, redirect the browser onward
 */
import { computed, onMounted, ref } from 'vue'
import AuthShell from '../components/AuthShell.vue'
import { runFlow } from '../api'

const props = defineProps({
  flowSlug: { type: String, default: 'dh-login' },
  query: { type: String, default: '' },
})

const emit = defineEmits(['done'])

const challenge = ref(null)
const loading = ref(true)
const submitting = ref(false)
const pageError = ref('')
const identifier = ref('')
const password = ref('')
const showPassword = ref(false)

const stage = computed(() => challenge.value?.component ?? null)
const errors = computed(() => challenge.value?.response_errors ?? {})

function errorMessages() {
  const out = []
  for (const list of Object.values(errors.value)) {
    for (const e of list ?? []) out.push(typeof e === 'string' ? e : e.string)
  }
  return out
}

function handleRedirect(ch) {
  if (ch.component === 'xak-flow-redirect' && ch.final_redirect) {
    const to = ch.to ?? '/'
    // A same-origin path means Authentik wants another executor hop; hand it
    // back to the parent so it can keep the chain going.
    emit('done', { location: to })
    return true
  }
  if (ch.component === 'xak-flow-redirect' && ch.to) {
    emit('done', { location: ch.to })
    return true
  }
  return false
}

async function submitIdentification() {
  submitting.value = true
  pageError.value = ''
  try {
    const next = await runFlow(props.flowSlug, '', {
      component: 'ak-stage-identification',
      uid_field: identifier.value,
    })
    challenge.value = next
    if (!handleRedirect(next)) return
  } catch (e) {
    pageError.value = e.message
  } finally {
    submitting.value = false
  }
}

async function submitPassword() {
  submitting.value = true
  pageError.value = ''
  try {
    const next = await runFlow(props.flowSlug, '', {
      component: 'ak-stage-password',
      password: password.value,
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
  <AuthShell subtitle="Masuk untuk melanjutkan">
    <div v-if="loading" class="py-8 text-center text-sm text-slate-500">Memuat…</div>

    <div v-else-if="pageError" class="rounded-lg border border-rose-200 bg-rose-50 p-4">
      <p class="text-sm font-medium text-rose-800">Gagal memuat halaman masuk</p>
      <p class="mt-1 text-xs text-rose-700">{{ pageError }}</p>
    </div>

    <!-- Identification -->
    <form v-else-if="stage === 'ak-stage-identification'" class="mt-2 space-y-4" @submit.prevent="submitIdentification">
      <div>
        <label for="identifier" class="block text-sm font-medium text-slate-700">
          Username atau email
        </label>
        <input
          id="identifier"
          v-model="identifier"
          type="text"
          autocomplete="username"
          required
          autofocus
          class="mt-1.5 w-full rounded-lg border border-slate-300 px-3 py-2 text-sm outline-none transition focus:border-indigo-500 focus:ring-2 focus:ring-indigo-100"
        />
      </div>

      <ul v-if="errorMessages().length" class="space-y-1 rounded-lg bg-rose-50 p-3">
        <li v-for="(m, i) in errorMessages()" :key="i" class="text-xs text-rose-700">{{ m }}</li>
      </ul>

      <button
        type="submit"
        :disabled="submitting"
        class="w-full rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-indigo-700 disabled:opacity-60"
      >
        {{ submitting ? 'Memproses…' : 'Lanjut' }}
      </button>
    </form>

    <!-- Password -->
    <form v-else-if="stage === 'ak-stage-password'" class="mt-2 space-y-4" @submit.prevent="submitPassword">
      <div v-if="challenge.pending_user" class="rounded-lg bg-slate-50 p-3 text-sm text-slate-700">
        Masuk sebagai <span class="font-medium">{{ challenge.pending_user }}</span>
      </div>

      <div>
        <label for="password" class="block text-sm font-medium text-slate-700">Kata sandi</label>
        <div class="relative mt-1.5">
          <input
            id="password"
            v-model="password"
            :type="showPassword ? 'text' : 'password'"
            autocomplete="current-password"
            required
            autofocus
            class="w-full rounded-lg border border-slate-300 px-3 py-2 pr-16 text-sm outline-none transition focus:border-indigo-500 focus:ring-2 focus:ring-indigo-100"
          />
          <button
            type="button"
            @click="showPassword = !showPassword"
            class="absolute inset-y-0 right-0 px-3 text-xs font-medium text-indigo-600 hover:text-indigo-700"
          >
            {{ showPassword ? 'Sembunyi' : 'Lihat' }}
          </button>
        </div>
      </div>

      <ul v-if="errorMessages().length" class="space-y-1 rounded-lg bg-rose-50 p-3">
        <li v-for="(m, i) in errorMessages()" :key="i" class="text-xs text-rose-700">{{ m }}</li>
      </ul>

      <button
        type="submit"
        :disabled="submitting"
        class="w-full rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-indigo-700 disabled:opacity-60"
      >
        {{ submitting ? 'Memproses…' : 'Masuk' }}
      </button>
    </form>

    <!-- Any stage this SPA does not render yet -->
    <div v-else class="mt-2 rounded-lg border border-amber-200 bg-amber-50 p-4">
      <p class="text-sm font-medium text-amber-900">Tahap belum didukung</p>
      <p class="mt-1 font-mono text-xs text-amber-800">{{ stage }}</p>
    </div>
  </AuthShell>
</template>