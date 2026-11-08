import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it, vi } from 'vitest'
import * as client from '@/api/client'
import { PLATFORM_TAGS, platformName } from '@/api/platforms'
import { pinFromSearch, normalizePin, pinComplete, PIN_LENGTH } from '@/lib/pin'
import { saveWarning } from '@/lib/saveWarning'
import { groupStates, stampLabel } from '@/lib/states'

const storage = { setItem: () => {}, removeItem: () => {}, getItem: () => null }
const src = (...parts: string[]) => readFileSync(resolve(__dirname, '../..', ...parts), 'utf8')

describe('platforms', () => {
  it('lists GB GBC GBA GBAV', () => {
    expect([...PLATFORM_TAGS]).toEqual(['GB', 'GBC', 'GBA', 'GBAV'])
  })

  it('names each tag', () => {
    expect(platformName('GBA')).toBe('Game Boy Advance')
    expect(platformName('GBAV')).toBe('Game Boy Advance Video')
    expect(platformName('GB')).toBe('Game Boy')
    expect(platformName('GBC')).toBe('Game Boy Color')
  })
})

describe('saves warning', () => {
  it('shows when resumeCores is not empty', () => {
    expect(saveWarning(['mgba'])).toBe(
      'This game has a resume state, which loads instead of this save. Hold A when you start the game to use this save.',
    )
  })

  it('is absent when resumeCores is empty', () => {
    expect(saveWarning([])).toBeNull()
    expect(saveWarning(undefined)).toBeNull()
  })

  it('reaches the page from the upload reply', () => {
    expect(src('components/game/SavesTab.vue')).toContain('saveWarning(result.resumeCores)')
  })
})

describe('login', () => {
  it('reads the pin from the query string', () => {
    expect(pinFromSearch('?pin=AB12CD')).toBe('AB12CD')
    expect(pinFromSearch('?host=x&pin=AB12CD')).toBe('AB12CD')
  })

  it('finds no pin when there is none', () => {
    expect(pinFromSearch('')).toBeNull()
    expect(pinFromSearch('?pin=')).toBeNull()
  })

  it('logs straight in and clears the query', () => {
    const login = src('views/LoginView.vue')
    expect(login).toContain('pinFromSearch(window.location.search)')
    expect(login).toContain("window.history.replaceState({}, '', window.location.pathname)")
  })

  it('takes a 4 digit pin through the normalizer and the auto-login condition', () => {
    expect(PIN_LENGTH).toBe(4)
    const digits = normalizePin('4821')
    expect(digits).toEqual(['4', '8', '2', '1'])
    expect(pinComplete(digits)).toBe(true)
  })

  it('strips non digits and rejects a short pin', () => {
    expect(normalizePin('48-2x1')).toEqual(['4', '8', '2', '1'])
    expect(pinComplete(normalizePin('48'))).toBe(false)
    expect(normalizePin('123456')).toHaveLength(4)
  })

  it('signs requests as the slot user', () => {
    expect(client.BASIC_USER).toBe('slot')
  })
})

describe('carts', () => {
  it('shows the cart image lazily on the grid', () => {
    const cover = src('components/game/GameCover.vue')
    expect(cover).toContain('props.game.cartUrl')
    expect(cover).toContain('loading="lazy"')
    expect(cover).toContain('decoding="async"')
    expect(cover).not.toContain('game.artUrl')
  })

  it('shows the same image on the game page and reloads the row after art changes', () => {
    const view = src('views/GameDetailView.vue')
    expect(view).toContain('game.value?.cartUrl')
    expect(view).not.toContain('gameArtBlob')
    expect(view.match(/await reloadRow\(\)/g)).toHaveLength(3)
  })

  it('sends a stale PIN to login when the page checks its session', async () => {
    vi.stubGlobal('localStorage', storage)
    const out = vi.fn()
    client.setUnauthorizedHandler(out)
    client.setCredentials('1234')
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 401 })))
    await client.checkSession()
    vi.unstubAllGlobals()
    expect(out).toHaveBeenCalledOnce()
    expect(client.isAuthenticated()).toBe(false)
  })

  it('stays put when a session check cannot reach the device', async () => {
    vi.stubGlobal('localStorage', storage)
    const lost = vi.fn()
    client.setConnectionErrorHandler(lost)
    client.setCredentials('1234')
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('offline')))
    await client.checkSession()
    expect(lost).not.toHaveBeenCalled()
    expect(client.isAuthenticated()).toBe(true)
    client.clearCredentials()
    vi.unstubAllGlobals()
  })

  it('does not check a session before login', async () => {
    vi.stubGlobal('localStorage', storage)
    client.clearCredentials()
    const send = vi.fn()
    vi.stubGlobal('fetch', send)
    await client.checkSession()
    vi.unstubAllGlobals()
    expect(send).not.toHaveBeenCalled()
  })

  it('checks the session when the page comes back', () => {
    const app = src('App.vue')
    expect(app).toContain("'visibilitychange'")
    expect(app).toContain("'focus'")
    expect(app).toContain('checkSession')
  })

  it('keeps a failed image fetch away from the connection handler', async () => {
    const lost = vi.fn()
    client.setConnectionErrorHandler(lost)
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('reset')))
    await expect(client.imageBlob('/x', new AbortController().signal)).rejects.toThrow('ConnectionError')
    expect(lost).not.toHaveBeenCalled()
    vi.unstubAllGlobals()
  })
})

describe('states', () => {
  const cores = [
    {
      id: 'mgba',
      core: 'mGBA',
      resume: true,
      states: [
        { stamp: '2026-10-29_10-00-00', size: 10 },
        { stamp: '2026-10-30_09-15-30', size: 12 },
      ],
    },
    { id: 'gpsp', core: 'gpSP', resume: false, states: [] },
    { id: 'sameboy', core: 'SameBoy', resume: false, states: [{ stamp: '2026-10-28_08-00-00', size: 5 }] },
  ]

  it('group by core, newest first, dropping empty cores', () => {
    const groups = groupStates(cores)
    expect(groups.map(g => g.id)).toEqual(['mgba', 'sameboy'])
    expect(groups[0]!.states.map(s => s.stamp)).toEqual(['2026-10-30_09-15-30', '2026-10-29_10-00-00'])
    expect(groups[0]!.resume).toBe(true)
  })

  it('keeps a core that only has a resume state', () => {
    expect(groupStates([{ id: 'mgba', core: 'mGBA', resume: true, states: [] }])).toHaveLength(1)
  })

  it('reads a stamp as a date and time', () => {
    expect(stampLabel('2026-10-30_09-15-30')).toBe('2026-10-30 09:15:30')
  })

  it('sends a save as bytes because Safari hangs on a file body', async () => {
    const send = vi.fn(async (_path: string, _init: RequestInit) => new Response('{"ok":true,"resumeCores":[]}'))
    vi.stubGlobal('fetch', send)
    await client.uploadGameSave('GBA', 8, new File([new Uint8Array(4)], 'x.sav'))
    vi.unstubAllGlobals()
    expect(send.mock.calls[0]?.[1].body).toBeInstanceOf(ArrayBuffer)
  })

  it('picks a save with an input on the page and no type filter', () => {
    const tab = src('components/game/SavesTab.vue')
    expect(tab).toContain('type="file"')
    expect(tab).not.toContain("createElement('input')")
    expect(tab).not.toContain('accept')
  })

  it('picks a custom label with an input on the page and no type filter', () => {
    const view = src('views/GameDetailView.vue')
    expect(view).toContain('type="file"')
    expect(view).not.toContain("createElement('input')")
    expect(view).not.toContain('accept')
  })

  it('picks a guide with an input on the page and no type filter', () => {
    const tab = src('components/game/GuidesTab.vue')
    expect(tab).toContain('type="file"')
    expect(tab).not.toContain("createElement('input')")
    expect(tab).not.toContain('accept')
  })

  it('has no state upload control', () => {
    expect('uploadGameState' in client).toBe(false)
    const tab = src('components/game/StatesTab.vue')
    expect(tab).not.toContain('type="file"')
    expect(tab).not.toContain("type = 'file'")
    expect(tab).not.toContain('uploadGameState')
  })
})
