import { createI18n } from 'vue-i18n'
import common from './en/common'
import dashboard from './en/dashboard'
import tools from './en/tools'
import login from './en/login'
import platform from './en/platform'
import game from './en/game'
import browse from './en/browse'
import dialogs from './en/dialogs'
import fileSlot from './en/fileSlot'

const en = { common, dashboard, tools, login, platform, game, browse, dialogs, fileSlot }

export const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en },
})
