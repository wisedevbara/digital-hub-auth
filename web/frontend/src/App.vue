<script setup>
/**
 * Digital Hub IdP interface.
 *
 * Instead of Authentik's built-in pages, this SPA asks the IdP where /authorize
 * would send the browser and then drives that flow through the flow executor
 * API, rendering each challenge itself.
 *
 * Multi-client: this file contains NO client-specific values. Which app is
 * asking arrives as `client_id` in the authorize query, and everything else
 * (flow slugs, branding, recovery URL) is resolved from /api/config. Adding a
 * second application + provider means creating them in Authentik and optionally
 * adding one entry to IDP_CLIENTS - no change needed here.
 *
 * Flow chain (verified against authentik 2026.8.3):
 *   /authorize -> /if/flow/<login-flow> (or <consent-flow>)
 *   identification -> password -> redirect
 *   consent -> redirect to ?query=/application/o/authorize/...
 *   final redirect -> client callback with ?code=...
 */
import { onMounted, ref } from 'vue'
import Login from './views/Login.vue'
import Consent from './views/Consent.vue'
import Denied from './views/Denied.vue'
import OidcError from './views/OidcError.vue'
import {
  getAuthorizeEntry,
  getConfig,
  parseFlowSlug,
  readClientId,
  recoverAuthorizeQuery,
  saveAuthorizeQuery,
} from './api'

const DEFAULT_CONFIG = {
  login_flow: 'dh-login',
  consent_flow: 'dh-consent',
  start_url: '/',
  brand_name: 'Digital Hub',
  brand_subtitle: '',
  client_name: '',
}

const view = ref('loading')
const config = ref(DEFAULT_CONFIG)
const clientId = ref('')
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
    goToStart()
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
  view.value = isConsentFlow(nextSlug) ? 'consent' : 'login'
}

/**
 * Consent vs login is decided by the flow slug the IdP actually redirected to,
 * not by a hardcoded name, so a provider bound to `dh-consent-strict` works too.
 */
function isConsentFlow(slug) {
  return slug.includes('consent')
}

/**
 * Last resort when there is no authorize request to finish. Sends the browser to
 * the configured start page instead of assuming one client app's /login route,
 * which is what broke as soon as a second app on another origin existed.
 */
function goToStart() {
  const target = config.value.start_url || '/'
  window.location.href = target
}

function extractQuery(location) {
  const m = location.match(/[?&]query=([^&]+)/)
  return m ? decodeURIComponent(m[1]) : ''
}

function handleFlowDone(payload) {
  follow(payload.location)
}

function fail(code, description) {
  errorCode.value = code
  errorDescription.value = description
  view.value = 'error'
}

onMounted(async () => {
  readQueryParams()

  if (errorCode.value) {
    view.value = ['access_denied', 'forbidden', 'unauthorized_client'].includes(errorCode.value)
      ? 'denied'
      : 'error'
    return
  }

  clientId.value = readClientId() ?? ''

  try {
    config.value = { ...DEFAULT_CONFIG, ...(await getConfig(clientId.value)) }
  } catch {
    // Config only supplies branding and slug defaults. The built-in defaults
    // keep the flow working, so a failure here must not abort the login.
    config.value = DEFAULT_CONFIG
  }

  try {
    // The authorize query must survive to this point. It normally arrives in
    // the URL, but a refresh or a direct visit to the SPA loses it, and an
    // empty query makes authentik answer 404 with no redirect target.
    let qs = window.location.search.replace(/^\?/, '')

    if (!qs.includes('client_id=')) {
      qs = await recoverAuthorizeQuery(clientId.value)
      if (!qs) {
        fail(
          'missing_authorize_query',
          'Permintaan otorisasi tidak ditemukan. Buka lagi halaman aplikasi untuk memulai login.',
        )
        return
      }
      // Reflect it so a refresh keeps working.
      window.history.replaceState({}, '', `/?${qs}`)
    }

    // Keep a copy scoped to this client_id so a later refresh resumes this
    // exact request even after the browser lands back on /idp/ without params.
    saveAuthorizeQuery(clientId.value, qs)

    // Pass the raw query string through untouched. Re-encoding it would turn the
    // "+" in "scope=openid+email+profile" into a literal plus or a space and
    // authentik would reject the request.
    const { location } = await getAuthorizeEntry(qs)

    if (!location) {
      // No redirect target: either the IdP session is not established yet, or
      // this client_id is unknown to the IdP. Either way the browser must go
      // back to the app that started the authorization to restart it.
      goToStart()
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
    view.value = isConsentFlow(slug) ? 'consent' : 'login'
  } catch (e) {
    fail('idp_unreachable', e.message)
  }
})
</script>

<template>
  <div v-if="view === 'loading'" class="flex min-h-screen items-center justify-center bg-slate-100">
    <p class="text-sm text-slate-500">Memuat…</p>
  </div>

  <Login
    v-else-if="view === 'login'"
    :flow-slug="flowSlug ?? config.login_flow"
    :query="query"
    :brand-name="config.brand_name"
    :client-name="config.client_name"
    @done="handleFlowDone"
  />

  <Consent
    v-else-if="view === 'consent'"
    :flow-slug="flowSlug ?? config.consent_flow"
    :query="query"
    :brand-name="config.brand_name"
    :client-name="config.client_name"
    @done="handleFlowDone"
  />

  <Denied
    v-else-if="view === 'denied'"
    :error="errorCode"
    :error-description="errorDescription"
    :brand-name="config.brand_name"
    :client-name="config.client_name"
    @retry="goToStart"
  />

  <OidcError
    v-else
    :error="errorCode || 'server_error'"
    :error-description="errorDescription"
    :detail="errorDetail"
    :brand-name="config.brand_name"
    :client-name="config.client_name"
    @retry="goToStart"
  />
</template>