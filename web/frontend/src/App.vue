<script setup>
/**
 * Digital Hub IdP interface.
 *
 * Instead of Authentik's built-in pages, this SPA asks the IdP where /authorize
 * would send the browser and then drives that flow through the flow executor
 * API, rendering each challenge itself.
 *
 * Flow chain (verified against authentik 2026.8.3):
 *   /authorize -> /if/flow/dh-login (or dh-consent)
 *   identification -> password -> redirect
 *   consent -> redirect to ?query=/application/o/authorize/...
 *   final redirect -> client callback with ?code=...
 */
import { onMounted, ref } from 'vue'
import Login from './views/Login.vue'
import Consent from './views/Consent.vue'
import Denied from './views/Denied.vue'
import OidcError from './views/OidcError.vue'
import { getAuthorizeEntry, parseFlowSlug, fetchAuthorizeQueryFromServer } from './api'

const view = ref('loading')
const flowSlug = ref(null)
const query = ref('')
const errorCode = ref('')
const errorDescription = ref('')
const errorDetail = ref('')

function readQueryParams() {
  const p = new URLSearchParams(window.location.search)
  errorCode.value = p.get('error') ?? ''
  errorDescription.value = p.get('error_description') ?? ''
  errorDetail.value = p.get('detail') ?? ''
}

/**
 * Follow a redirect target returned by the flow.
 * A relative executor path is another challenge hop; an absolute URL is the
 * real destination (the client app's callback).
 */
function follow(location) {
  if (!location || location === '/') {
    window.location.href = '/'
    return
  }

  if (/^https?:\/\//i.test(location)) {
    window.location.href = location
    return
  }

  const nextSlug = parseFlowSlug(location)
  if (!nextSlug) {
    window.location.href = location
    return
  }

  flowSlug.value = nextSlug
  query.value = extractQuery(location)
  view.value = nextSlug.includes('consent') ? 'consent' : 'login'
}

function extractQuery(location) {
  const m = location.match(/[?&]query=([^&]+)/)
  return m ? decodeURIComponent(m[1]) : ''
}

function handleFlowDone(payload) {
  follow(payload.location)
}

onMounted(async () => {
  readQueryParams()

  if (errorCode.value) {
    view.value = ['access_denied', 'forbidden', 'unauthorized_client'].includes(errorCode.value)
      ? 'denied'
      : 'error'
    return
  }

  try {
    // The authorize query must survive to this point. It normally arrives in
    // the URL, but a refresh or a direct visit to the SPA loses it, and an
    // empty query makes authentik answer 404 with no redirect target. The
    // server keeps the same query in the session, so use it as a fallback.
    let qs = window.location.search.replace(/^\?/, '')

    if (!qs.includes('client_id=')) {
      qs = await fetchAuthorizeQueryFromServer()
      if (!qs) {
        errorCode.value = 'missing_authorize_query'
        errorDescription.value =
          'Permintaan otorisasi tidak ditemukan. Buka lagi halaman aplikasi untuk memulai login.'
        view.value = 'error'
        return
      }
      // Reflect it so a refresh keeps working.
      window.history.replaceState({}, '', `/?${qs}`)
    }

    // Pass the raw query string through untouched. Re-encoding it would turn the
    // "+" in "scope=openid+email+profile" into a literal plus or a space and
    // authentik would reject the request.
    const { location } = await getAuthorizeEntry(qs)

    if (!location) {
      // No redirect target. This normally means the IdP session cookie has not
      // been established yet. Ask Laravel to (re)start the authorization,
      // which re-seeds both the session and the SPA URL, then let the browser
      // follow that redirect.
      window.location.href = '/login'

      return
    }

    const slug = parseFlowSlug(location)
    if (!slug) {
      // The browser is redirected to the client, so just send it there.
      window.location.href = location
      return
    }

    flowSlug.value = slug
    query.value = extractQuery(location)
    view.value = slug.includes('consent') ? 'consent' : 'login'
  } catch (e) {
    errorCode.value = 'idp_unreachable'
    errorDescription.value = e.message
    view.value = 'error'
  }
})
</script>

<template>
  <div v-if="view === 'loading'" class="flex min-h-screen items-center justify-center bg-slate-100">
    <p class="text-sm text-slate-500">Memuat…</p>
  </div>

  <Login
    v-else-if="view === 'login'"
    :flow-slug="flowSlug ?? 'dh-login'"
    :query="query"
    @done="handleFlowDone"
  />

  <Consent
    v-else-if="view === 'consent'"
    :flow-slug="flowSlug ?? 'dh-consent'"
    :query="query"
    @done="handleFlowDone"
  />

  <Denied
    v-else-if="view === 'denied'"
    :error="errorCode"
    :error-description="errorDescription"
  />

  <OidcError
    v-else
    :error="errorCode || 'server_error'"
    :error-description="errorDescription"
    :detail="errorDetail"
  />
</template>