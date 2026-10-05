/**
 * Client for the Rust API that proxies Authentik's flow executor.
 *
 * The SPA is served under /idp by nginx, so API calls go to /idp/api.
 * Cookies are sent with every call (credentials: 'include') because the
 * Authentik session cookie is what carries the flow state between stages.
 */
const API = '/idp/api'

async function request(path, options = {}) {
  // Guard against a hung fetch leaving the SPA stuck on "loading" forever.
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), 15000)

  try {
    const res = await fetch(`${API}${path}`, {
      credentials: 'include',
      signal: controller.signal,
      headers: { 'Content-Type': 'application/json' },
      ...options,
    })

    const text = await res.text()
    let data = null
    try {
      data = text ? JSON.parse(text) : null
    } catch {
      data = { detail: text }
    }

    if (!res.ok) {
      const err = new Error(data?.detail || `HTTP ${res.status}`)
      err.status = res.status
      err.payload = data
      throw err
    }

    return data
  } finally {
    clearTimeout(timer)
  }
}

// NOTE: API already includes "/idp/api", so these paths must not repeat "/api".
//
// The IdP serves many client apps (Laravel today, Next.js later). Everything the
// SPA needs to know about "which app is asking" comes from /config, resolved by
// client_id - nothing about a specific client is baked into this file.
export function getConfig(clientId) {
  const qs = clientId ? `?client_id=${encodeURIComponent(clientId)}` : ''
  return request(`/config${qs}`)
}

/**
 * Key the stored authorize query per client.
 *
 * With several apps on one IdP, a single shared key would let app B's
 * authorize request be resumed by app A's page. The client_id is the only
 * reliable discriminator, so it goes in the key.
 */
function queryKey(clientId) {
  return `dh:authorize_query:${clientId || 'anonymous'}`
}

function saveAuthorizeQuery(clientId, query) {
  try {
    sessionStorage.setItem(queryKey(clientId), query)
  } catch {
    // Private mode / storage disabled: the URL still carries the query, so the
    // only loss is the refresh fallback.
  }
}

function loadAuthorizeQuery(clientId) {
  try {
    return sessionStorage.getItem(queryKey(clientId))
  } catch {
    return null
  }
}

function readClientId() {
  return new URLSearchParams(window.location.search).get('client_id')
}

/**
 * Ask the IdP where /authorize would send the browser, so we know whether to
 * run the login flow or the consent flow.
 */
export function getAuthorizeEntry(query) {
  return request(`/authorize/entry?${query}`)
}

/**
 * Run a flow stage.
 *
 * The `query=` parameter must always be present (even empty): without it
 * Authentik's FlowPlanner re-issues the current challenge instead of
 * advancing, and the submit looks like it silently did nothing.
 */
export function runFlow(flowSlug, query, body) {
  const qs = `?query=${encodeURIComponent(query ?? '')}`
  return request(`/flow/${flowSlug}${qs}`, {
    method: 'POST',
    body: JSON.stringify(body ?? {}),
  })
}

export function parseFlowSlug(location) {
  if (!location) return null
  const m = location.match(/\/if\/flow\/([a-z0-9-]+)/i)
  return m ? m[1] : null
}

/**
 * Recover the authorize query when the URL no longer carries it (refresh, or
 * the SPA opened directly).
 *
 * Order matters. The browser's own copy is authoritative and works for every
 * client app on any origin - that is the multi-client answer. Laravel's
 * /auth/authorize-query is only consulted when storage is unavailable AND the
 * SPA happens to be same-origin with a Laravel app, so it stays as a last
 * resort for the first client without becoming a hard dependency.
 */
export async function recoverAuthorizeQuery(clientId) {
  const stored = loadAuthorizeQuery(clientId)
  if (stored) return stored

  return fetchAuthorizeQueryFromLaravel()
}

/** Legacy single-client fallback. Returns null when there is no Laravel app. */
async function fetchAuthorizeQueryFromLaravel() {
  try {
    const res = await fetch('/auth/authorize-query', {
      credentials: 'same-origin',
      headers: { Accept: 'application/json' },
    })

    if (!res.ok) return null

    const data = await res.json()
    return data?.query ?? null
  } catch {
    return null
  }
}

export { readClientId, saveAuthorizeQuery }