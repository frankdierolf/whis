/// <reference types="vite/client" />

declare module '*.vue' {
  import type { createVaporApp } from 'vue'
  type VaporRoot = Parameters<typeof createVaporApp>[0]
  const component: VaporRoot
  export default component
}
