// Optional native sysinfo companion. Never store the access token in localStorage
// or the portable state. Browser-only mode remains fully functional.
export const MONITOR_URL = 'http://127.0.0.1:47831/v1/metrics'

export function validateNativeMetrics(value) {
  if (!value || value.schema !== 1 || value.source !== '4c-native-sysinfo') {
    throw new Error('Unexpected local monitor response.')
  }
  const fields = ['timestamp_ms', 'total_memory_bytes', 'available_memory_bytes', 'used_memory_bytes', 'total_swap_bytes', 'used_swap_bytes', 'logical_cpus', 'cpu_usage_percent']
  for (const key of fields) {
    if (typeof value[key] !== 'number' || !Number.isFinite(value[key]) || value[key] < 0) {
      throw new Error(`Invalid native monitor field: ${key}`)
    }
  }
  if (!value.total_memory_bytes || value.available_memory_bytes > value.total_memory_bytes || value.cpu_usage_percent > 100 || value.logical_cpus < 1) {
    throw new Error('Native monitor returned inconsistent system measurements.')
  }
  if (Math.abs(Date.now() - value.timestamp_ms) > 120000) throw new Error('Local monitor response is stale.')
  return value
}

export function createSystemMonitor({ fetchImpl = (...args) => globalThis.fetch(...args), url = MONITOR_URL } = {}) {
  let accessToken = ''
  let last = null
  let status = 'disconnected'
  let error = ''
  let generation = 0
  const getState = () => ({ status, last, error })
  const disconnect = () => {
    generation++
    accessToken = ''
    last = null
    error = ''
    status = 'disconnected'
    return getState()
  }
  const request = async (token, thisGeneration) => {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), 4000)
    try {
      const response = await fetchImpl(url, {
        method: 'GET', mode: 'cors', credentials: 'omit', cache: 'no-store',
        headers: { Authorization: `Bearer ${token}` }, signal: controller.signal,
      })
      if (!response.ok) throw new Error(response.status === 401 ? 'Incorrect monitor access token.' : `Monitor returned HTTP ${response.status}.`)
      const metrics = validateNativeMetrics(await response.json())
      if (thisGeneration !== generation) return getState()
      last = metrics
      status = 'connected'
      error = ''
      return getState()
    } finally { clearTimeout(timer) }
  }
  const connect = async (token) => {
    if (!/^[0-9a-f]{32}$/.test(String(token).trim())) {
      status = 'disconnected'
      error = 'Paste the 32-character access token shown by the native monitor.'
      return getState()
    }
    const thisGeneration = ++generation
    status = 'connecting'; error = ''; last = null
    try {
      const result = await request(String(token).trim(), thisGeneration)
      if (thisGeneration === generation && status === 'connected') accessToken = String(token).trim()
      return result
    } catch (cause) {
      if (thisGeneration === generation) {
        status = 'disconnected'
        error = cause instanceof TypeError || cause?.name === 'AbortError'
          ? 'Cannot reach the local monitor. Check that it is running, the browser allows loopback access, and the URL is not blocked.'
          : String(cause?.message || cause)
        accessToken = ''
      }
      return getState()
    }
  }
  const refresh = async () => {
    if (status !== 'connected' || !accessToken) return getState()
    const current = generation
    try { return await request(accessToken, current) }
    catch (cause) {
      if (current === generation) {
        status = 'unavailable'
        error = String(cause?.message || cause)
        last = null // Never show stale RAM as if it were live.
      }
      return getState()
    }
  }
  return { connect, refresh, disconnect, getState }
}
