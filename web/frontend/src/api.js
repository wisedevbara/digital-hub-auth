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
export const getConfig = () => request('/config')

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
 * Recover the authorize query from the Laravel app.
 *
 * The SPA can lose the query on refresh or when opened directly. Laravel
 * keeps the same query in the session, so ask it for a fresh copy. Both are
 * now same-origin (nginx serves the SPA under /idp), so a relative URL is
 * enough and the session cookie is sent normally.
 */
export async function fetchAuthorizeQueryFromServer() {
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