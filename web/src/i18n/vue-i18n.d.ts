import 'vue'

declare module 'vue' {
  interface ComponentCustomProperties {
    $t: (key: string, ...args: unknown[]) => string
  }
}

export {}
