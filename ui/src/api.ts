export type Rt = { range: number; strokeOn: number; strokeOff: number; beyond: boolean }
export type Key = {
  index: number; row: number; col: number; stabilized: boolean
  keycodes: [number, number, number]; label: string; fnLabel: string
  pointOn: number; pointOff: number; rapidTrigger: boolean; rt: Rt
}
export type Socd = { mode: number; key1: number; key2: number }
export type KbInfo = {
  model: string; deviceName: string; serial: string; firmware: string
  protocol: number; rows: number; cols: number; activeProfile: number
}
export type KbData = {
  info: KbInfo
  state: { profile: number; keys: Key[]; socd: Socd[]; holdTime: { on: number; off: number } | null }
}
export type MouseData = {
  model: string; connection: string; firmware: string; battery: number | null; charging: boolean
  activeProfile: number; profile: number; pollingRate: number; debounceMs: number; sleepSec: number
  lod: number; angleSnap: boolean; motionSync: boolean; rippleControl: boolean; competitionMode: boolean
  dpiXy: boolean; angleTune: number; dpiStages: { x: number; y: number }[]; activeStage: number
}
export type Status = {
  keyboard: { connected: boolean; info?: KbInfo; error?: string }
  mouse: { connected: boolean; info?: { connection: string; battery: number | null; charging: boolean }; error?: string }
  version: string
}
export type KeyCode = { code: number; name: string; label: string }

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body !== undefined || method !== 'GET' ? { 'Content-Type': 'application/json', 'X-Devhub': '1' } : {},
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })
  const j = await res.json()
  if (!j.ok) throw new Error(j.error ?? '不明なエラー')
  return j.data as T
}

export const api = {
  get: <T,>(p: string) => call<T>('GET', p),
  patch: <T,>(p: string, b: unknown) => call<T>('PATCH', p, b),
  post: <T,>(p: string, b: unknown) => call<T>('POST', p, b),
  put: <T,>(p: string, b: unknown) => call<T>('PUT', p, b),
  del: <T,>(p: string) => call<T>('DELETE', p),
}

export const PROFILES = ['ベース', '一時', 'MEM1', 'MEM2', 'MEM3']
export const mm = (v: number) => `${(v / 1000).toFixed(2)}mm`
