import { ref } from 'vue'

export interface FileEntry {
  name: string
  type: 'file' | 'dir'
  size: number
}

export interface ListResponse {
  path: string
  entries: FileEntry[]
}

const token = ref('')
const authenticated = ref(false)

let onUnauthorized: (() => void) | null = null
let onConnectionError: (() => void) | null = null

export function setUnauthorizedHandler(handler: () => void) {
  onUnauthorized = handler
}

export function setConnectionErrorHandler(handler: () => void) {
  onConnectionError = handler
}

export const BASIC_USER = 'slot'
export const PIN_KEY = 'slot_pin'

export function setCredentials(pin: string) {
  token.value = btoa(`${BASIC_USER}:${pin}`)
  authenticated.value = true
  localStorage.setItem(PIN_KEY, pin)
}

export function restoreCredentials(): boolean {
  const pin = localStorage.getItem(PIN_KEY)
  if (!pin) return false
  setCredentials(pin)
  return true
}

export function isAuthenticated() {
  return authenticated.value
}

export function clearCredentials() {
  token.value = ''
  authenticated.value = false
  localStorage.removeItem(PIN_KEY)
}

export function buildPath(resource: string, ...segments: (string | undefined)[]): string {
  const encoded = [resource, ...segments.filter(Boolean)].map(p => encodeURIComponent(p!))
  return `/api/${encoded.join('/')}`
}

async function request(path: string, init?: RequestInit, quiet = false): Promise<Response> {
  let res: Response
  try {
    const headers: Record<string, string> = { ...(init?.headers as Record<string, string> | undefined) }
    if (token.value) headers.Authorization = `Basic ${token.value}`
    res = await fetch(path, { ...init, headers })
  } catch {
    if (!quiet) onConnectionError?.()
    throw new Error('ConnectionError')
  }
  if (res.status === 401) {
    clearCredentials()
    onUnauthorized?.()
    throw new Error('Unauthorized')
  }
  return res
}

let checkingSession = false

export async function checkSession(): Promise<void> {
  if (!authenticated.value || checkingSession) return
  checkingSession = true
  try {
    await request('/api/tags', undefined, true)
  } catch {
  } finally {
    checkingSession = false
  }
}

export async function getTags(): Promise<string[]> {
  const res = await request('/api/tags')
  if (!res.ok) throw new Error(`getTags ${res.status}`)
  return (await res.json()).tags
}

export interface GameRow {
  id: number
  rom: string
  displayName: string
  sortKey: string
  path: string
  hasArt: boolean
  cartUrl: string
  savesCount: number
  statesCount: number
  guidesCount: number
}

export interface PlatformGames {
  platform: string
  displayName: string
  games: GameRow[]
}

export interface GameDetail extends GameRow {
  platform: string
  platformDisplayName: string
  size: number
  modified: number
}

export async function getGames(tag: string): Promise<PlatformGames> {
  const res = await request(`/api/games/${encodeURIComponent(tag)}`)
  if (!res.ok) throw new Error(`getGames ${res.status}`)
  return res.json()
}

export async function getGame(tag: string, id: number): Promise<GameDetail> {
  const res = await request(`/api/games/${encodeURIComponent(tag)}/${id}`)
  if (!res.ok) throw new Error(`getGame ${res.status}`)
  return res.json()
}

function gameBase(tag: string, id: number): string {
  return `/api/games/${encodeURIComponent(tag)}/${id}`
}

export interface GameFile {
  name: string
  size: number
  modified: number
}

export interface GameRomFile {
  name: string
  path: string
  size: number
  modified: number
}

export class StaleListError extends Error {
  constructor() {
    super('StaleList')
  }
}

export async function imageBlob(path: string, signal: AbortSignal): Promise<string | null> {
  const res = await request(path, { signal }, true)
  if (!res.ok) return null
  return URL.createObjectURL(await res.blob())
}

export async function resourceFileBlob(resource: string, tag: string, name: string): Promise<string | null> {
  const res = await request(buildPath(resource, tag, name))
  if (!res.ok) return null
  return URL.createObjectURL(await res.blob())
}

export async function fileBlob(resource: string, ...segments: string[]): Promise<{ url: string; type: string }> {
  const res = await request(buildPath(resource, ...segments))
  if (!res.ok) throw new Error(`fileBlob ${res.status}`)
  const blob = await res.blob()
  return { url: URL.createObjectURL(blob), type: blob.type }
}

export async function uploadGameArt(tag: string, id: number, file: File): Promise<void> {
  const res = await request(`${gameBase(tag, id)}/art?name=${encodeURIComponent(file.name)}`, {
    method: 'POST',
    body: await file.arrayBuffer(),
  })
  if (!res.ok) throw new Error(`uploadGameArt ${res.status}`)
}

export async function fetchGameArt(tag: string, id: number): Promise<boolean> {
  const res = await request(`${gameBase(tag, id)}/fetch-label`, { method: 'POST' })
  if (res.status === 404) return false
  if (!res.ok) throw new Error(`fetchGameArt ${res.status}`)
  return true
}

export async function deleteGameArt(tag: string, id: number): Promise<void> {
  const res = await request(`${gameBase(tag, id)}/art`, { method: 'DELETE' })
  if (!res.ok) throw new Error(`deleteGameArt ${res.status}`)
}

export async function listGameRoms(tag: string, id: number): Promise<GameRomFile[]> {
  const res = await request(`${gameBase(tag, id)}/roms`)
  if (!res.ok) throw new Error(`listGameRoms ${res.status}`)
  return (await res.json()).files
}

export async function downloadGameRomFile(tag: string, id: number, path: string, filename: string): Promise<void> {
  await downloadFile(`${gameBase(tag, id)}/rom?file=${encodeURIComponent(path)}`, filename)
}

export async function deleteGame(tag: string, id: number, rom: string, purge: boolean): Promise<void> {
  const query = `?rom=${encodeURIComponent(rom)}${purge ? '&purge=true' : ''}`
  const res = await request(`${gameBase(tag, id)}${query}`, { method: 'DELETE' })
  if (res.status === 409) throw new StaleListError()
  if (!res.ok) throw new Error(`deleteGame ${res.status}`)
}

export async function renameGame(tag: string, id: number, name: string): Promise<void> {
  const res = await request(`${gameBase(tag, id)}/rename?name=${encodeURIComponent(name)}`, { method: 'POST' })
  if (!res.ok) throw new Error(`renameGame ${res.status}`)
}

export async function listGameSaves(tag: string, id: number): Promise<GameFile[]> {
  const res = await request(`${gameBase(tag, id)}/saves`)
  if (!res.ok) throw new Error(`listGameSaves ${res.status}`)
  return (await res.json()).files
}

export async function downloadGameSave(tag: string, id: number, name: string): Promise<void> {
  await downloadFile(`${gameBase(tag, id)}/saves?file=${encodeURIComponent(name)}`, name)
}

export interface SaveUploadResult {
  ok: boolean
  resumeCores: string[]
}

export async function uploadGameSave(tag: string, id: number, file: File): Promise<SaveUploadResult> {
  const res = await request(`${gameBase(tag, id)}/saves?name=${encodeURIComponent(file.name)}`, {
    method: 'POST',
    body: await file.arrayBuffer(),
  })
  if (!res.ok) throw new Error(`uploadGameSave ${res.status}`)
  const data = await res.json()
  return { ok: data.ok === true, resumeCores: Array.isArray(data.resumeCores) ? data.resumeCores : [] }
}

export async function deleteGameSave(tag: string, id: number, name: string): Promise<void> {
  const res = await request(`${gameBase(tag, id)}/saves?file=${encodeURIComponent(name)}`, { method: 'DELETE' })
  if (!res.ok) throw new Error(`deleteGameSave ${res.status}`)
}

export async function listGameGuides(tag: string, id: number): Promise<GameFile[]> {
  const res = await request(`${gameBase(tag, id)}/guides`)
  if (!res.ok) throw new Error(`listGameGuides ${res.status}`)
  return (await res.json()).files
}

export async function downloadGameGuide(tag: string, id: number, name: string): Promise<void> {
  await downloadFile(`${gameBase(tag, id)}/guides?file=${encodeURIComponent(name)}`, name)
}

export async function gameGuideText(tag: string, id: number, name: string): Promise<string> {
  const res = await request(`${gameBase(tag, id)}/guides?file=${encodeURIComponent(name)}`)
  if (!res.ok) throw new Error(`gameGuideText ${res.status}`)
  return res.text()
}

export async function uploadGameGuide(tag: string, id: number, file: File): Promise<void> {
  const res = await request(`${gameBase(tag, id)}/guides?name=${encodeURIComponent(file.name)}`, {
    method: 'POST',
    body: await file.arrayBuffer(),
  })
  if (!res.ok) throw new Error(`uploadGameGuide ${res.status}`)
}

export async function deleteGameGuide(tag: string, id: number, name: string): Promise<void> {
  const res = await request(`${gameBase(tag, id)}/guides?file=${encodeURIComponent(name)}`, { method: 'DELETE' })
  if (!res.ok) throw new Error(`deleteGameGuide ${res.status}`)
}

export interface StateEntry {
  stamp: string
  size: number
}

export interface CoreStates {
  id: string
  core: string
  resume: boolean
  states: StateEntry[]
}

export async function listGameStates(tag: string, id: number): Promise<CoreStates[]> {
  const res = await request(`${gameBase(tag, id)}/states`)
  if (!res.ok) throw new Error(`listGameStates ${res.status}`)
  return (await res.json()).cores
}

function statePath(tag: string, id: number, core: string, stamp: string): string {
  return `${gameBase(tag, id)}/states/${encodeURIComponent(core)}/${encodeURIComponent(stamp)}`
}

export async function gameStateThumbBlob(tag: string, id: number, core: string, stamp: string): Promise<string | null> {
  const res = await request(`${statePath(tag, id, core, stamp)}/thumb`)
  if (!res.ok) return null
  return URL.createObjectURL(await res.blob())
}

export async function downloadGameState(tag: string, id: number, core: string, stamp: string, filename: string): Promise<void> {
  await downloadFile(statePath(tag, id, core, stamp), filename)
}

export async function deleteGameState(tag: string, id: number, core: string, stamp: string): Promise<void> {
  const res = await request(statePath(tag, id, core, stamp), { method: 'DELETE' })
  if (!res.ok) throw new Error(`deleteGameState ${res.status}`)
}

export async function listFiles(resource: string, ...subpath: (string | undefined)[]): Promise<ListResponse> {
  const res = await request(buildPath(resource, ...subpath))
  if (!res.ok) throw new Error(`listFiles ${res.status}`)
  return res.json()
}

export async function createFolder(resource: string, ...subpath: (string | undefined)[]): Promise<{ ok: boolean }> {
  const res = await request(buildPath(resource, ...subpath), { method: 'PUT' })
  return res.json()
}

export async function moveFile(resource: string, fromSegments: string[], to: string): Promise<{ ok: boolean }> {
  const res = await request(buildPath(resource, ...fromSegments), {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ to }),
  })
  return res.json()
}

export async function downloadToDisk(resource: string, segments: string[], filename: string): Promise<void> {
  await downloadFile(buildPath(resource, ...segments), filename)
}

export async function downloadFile(path: string, filename: string): Promise<void> {
  const res = await request(path)
  if (!res.ok) throw new Error(`download ${res.status}`)
  const blob = await res.blob()
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}

export async function deleteFile(resource: string, ...subpath: (string | undefined)[]): Promise<{ ok: boolean }> {
  const res = await request(buildPath(resource, ...subpath), { method: 'DELETE' })
  return res.json()
}

export interface Volume {
  id: string
  label: string
  totalBytes: number
  freeBytes: number
}

export async function listVolumes(): Promise<Volume[]> {
  const res = await request('/api/fs')
  const data = await res.json()
  return data.volumes
}

export function uploadFiles<T = { ok: boolean; files: string[] }>(
  resource: string,
  subpath: string[],
  files: File[],
  onProgress?: (pct: number) => void,
): { promise: Promise<T>; abort: () => void } {
  const path = buildPath(resource, ...subpath)

  const formData = new FormData()
  for (const file of files) {
    formData.append('file', file, file.name)
  }

  const xhr = new XMLHttpRequest()

  const promise = new Promise<T>((resolve, reject) => {
    xhr.open('POST', path)
    if (token.value) xhr.setRequestHeader('Authorization', `Basic ${token.value}`)

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable && onProgress) {
        onProgress(Math.round((e.loaded / e.total) * 100))
      }
    })

    xhr.addEventListener('load', () => {
      if (xhr.status === 200) {
        resolve(JSON.parse(xhr.responseText))
      } else {
        reject(new Error(`Upload failed: ${xhr.status}`))
      }
    })

    xhr.addEventListener('error', () => reject(new Error('Upload failed')))
    xhr.addEventListener('abort', () => reject(new Error('Upload cancelled')))
    xhr.send(formData)
  })

  return { promise, abort: () => xhr.abort() }
}
