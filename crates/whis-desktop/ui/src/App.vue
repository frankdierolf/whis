<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { settingsStore } from './stores/settings'

const route = useRoute()

// App info
const appVersion = '0.7.2'
const appRepo = 'https://github.com/frankdierolf/whis'
const appSite = 'https://whis.ink'

// Provide app info to AboutView via route meta or props
// For now, we'll pass them through router

// Window controls
const showCustomControls = ref(true)
const loaded = computed(() => settingsStore.state.loaded)
const windowVisible = computed(() => settingsStore.state.windowVisible)

// Current route name for navigation highlighting
const currentRoute = computed(() => route.name as string)

// Navigation items
const navItems = [
  { name: 'home', label: 'home', path: '/' },
  { name: 'shortcut', label: 'shortcut', path: '/shortcut' },
  { name: 'settings', label: 'settings', path: '/settings' },
  { name: 'presets', label: 'presets', path: '/presets' },
  { name: 'about', label: 'about', path: '/about' },
]

function isActive(name: string): boolean {
  return currentRoute.value === name
}

// Window controls
async function minimizeWindow() {
  await getCurrentWindow().minimize()
}

async function closeWindow() {
  try {
    // Fade out before hiding to prevent flicker on reopen
    settingsStore.setWindowVisible(false)
    await new Promise(r => setTimeout(r, 150)) // Wait for CSS transition

    // Flush pending settings before closing to prevent race condition with debounced auto-save
    await settingsStore.flush()

    const canReopen = await invoke<boolean>('can_reopen_window')
    if (canReopen) {
      await getCurrentWindow().hide()
    }
    else {
      await getCurrentWindow().close()
    }
  }
  catch {
    await getCurrentWindow().close()
  }
}

onMounted(async () => {
  showCustomControls.value = true
  await settingsStore.initialize()

  const currentWindow = getCurrentWindow()

  // Listen for window focus changes (handles reopen from tray)
  // Use double-RAF to ensure WebView has repainted before transitioning
  await currentWindow.onFocusChanged(({ payload: focused }) => {
    if (focused) {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          settingsStore.setWindowVisible(true)
        })
      })
    }
  })

  // Initial visibility transition (first load)
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      settingsStore.setWindowVisible(true)
    })
  })

  // Listen for tray quit event - flush settings before exit
  await listen('tray-quit-requested', async () => {
    await settingsStore.flush()
    await invoke('exit_app')
  })

  // Listen for window close event (Alt+F4, system close, etc.)
  await listen('window-close-requested', closeWindow)
})
</script>

<template>
  <div class="app" :class="{ loaded: loaded && windowVisible }">
    <div class="window">
      <!-- Sidebar -->
      <aside class="sidebar" data-tauri-drag-region>
        <div class="brand" data-tauri-drag-region>
          <span class="wordmark">whis</span>
        </div>

        <nav class="nav">
          <router-link
            v-for="item in navItems"
            :key="item.name"
            :to="item.path"
            class="nav-item"
            :class="{ active: isActive(item.name) }"
          >
            <span class="nav-marker" aria-hidden="true">{{ isActive(item.name) ? '>' : ' ' }}</span>
            <span>{{ item.label }}</span>
          </router-link>
        </nav>
      </aside>

      <!-- Content -->
      <main class="content">
        <!-- Title bar for dragging -->
        <div v-if="showCustomControls" class="titlebar" data-tauri-drag-region>
          <div class="window-controls">
            <button class="control-btn" title="Minimize" @click="minimizeWindow">
              <svg viewBox="0 0 10 10"><rect x="1" y="4.5" width="8" height="1" fill="currentColor" /></svg>
            </button>
            <button class="control-btn close" title="Close" @click="closeWindow">
              <svg viewBox="0 0 10 10"><path d="M1.5 1.5L8.5 8.5M8.5 1.5L1.5 8.5" stroke="currentColor" stroke-width="1.2" /></svg>
            </button>
          </div>
        </div>

        <!-- Router view with props for static data -->
        <router-view
          :app-version="appVersion"
          :app-site="appSite"
          :app-repo="appRepo"
        />
      </main>
    </div>
  </div>
</template>

<style>
/* Design tokens - matching whis.ink website */
:root {
  /* Background */
  --bg: hsl(0, 0%, 7%);
  --bg-weak: hsl(0, 0%, 11%);
  --bg-hover: hsl(0, 0%, 16%);
  --bg-strong: hsl(0, 0%, 100%);

  /* Text */
  --text: hsl(0, 0%, 80%);
  --text-weak: hsl(0, 0%, 62%);
  --text-strong: hsl(0, 0%, 100%);
  --text-inverted: hsl(0, 0%, 7%);

  /* Accent - gold */
  --accent: hsl(48, 100%, 60%);

  /* Border */
  --border: hsl(0, 0%, 24%);

  /* Icon */
  --icon: hsl(0, 0%, 55%);

  /* Functional */
  --recording: #ff4444;

  /* Typography */
  --font: "JetBrains Mono", "Fira Code", "SF Mono", ui-monospace, monospace;
  --line-height: 1.6;

  /* Layout */
  --field-label-width: 100px;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  height: 100%;
  overflow: hidden;
}

body {
  font-family: var(--font);
  font-size: 13px;
  line-height: var(--line-height);
  background: transparent;
  color: var(--text);
  -webkit-font-smoothing: antialiased;
}

#app {
  height: 100%;
}

::selection {
  background: var(--accent);
  color: var(--text-strong);
}

/* Shared styles for views */
.section {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 24px;
  overflow-y: auto;
  overflow-x: hidden;
}

.section-header {
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border);
}

.section-header h1 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-strong);
  margin-bottom: 4px;
}

.section-header p {
  font-size: 12px;
  color: var(--text-weak);
}

.section-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

/* Field */
.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field label {
  font-size: 11px;
  text-transform: lowercase;
  color: var(--text-weak);
}

/* Button - website style (white bg, dark text) */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 10px 20px;
  background: var(--bg-strong);
  border: none;
  border-radius: 4px;
  font-family: var(--font);
  font-size: 12px;
  font-weight: 500;
  color: var(--text-inverted);
  cursor: pointer;
  transition: all 0.15s ease;
  align-self: flex-start;
}

.btn:hover:not(:disabled) {
  background: hsl(0, 0%, 90%);
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-secondary {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text);
}

.btn-secondary:hover:not(:disabled) {
  background: var(--bg-weak);
  border-color: var(--text-weak);
}

.btn-link {
  background: none;
  border: none;
  padding: 0;
  font-family: var(--font);
  font-size: 12px;
  color: var(--text-strong);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
}

.btn-link:hover {
  color: var(--accent);
}

/* Notice */
.notice {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  min-width: 0;
}

.notice-marker {
  color: var(--accent);
  flex-shrink: 0;
}

.notice p {
  font-size: 12px;
  color: var(--text);
  line-height: 1.5;
  min-width: 0;
  word-wrap: break-word;
}

/* Hint */
.hint {
  font-size: 11px;
  color: var(--text-weak);
}

.hint a {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.hint a:hover {
  color: var(--text-strong);
}

/* Status */
.status {
  font-size: 11px;
  color: var(--accent);
  opacity: 0;
  transition: opacity 0.15s ease;
}

.status.visible {
  opacity: 1;
}

/* Screen reader only - visually hidden but accessible */
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}

/* Shared keyboard shortcut key display */
.keys {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 26px;
  padding: 0 8px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-family: var(--font);
  font-size: 11px;
  font-weight: 500;
  color: var(--accent);
}

/* Field row - horizontal label + input layout */
.field-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.field-row > label {
  width: var(--field-label-width);
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-weak);
}

.field-row > label.disabled {
  opacity: 0.5;
}

.field-row > input {
  flex: 1;
}

.field-row :deep(.custom-select) {
  flex: 1;
}

/* Sliding panel pattern */
.slide-panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 320px;
  background: var(--bg);
  border-left: 1px solid var(--border);
  transform: translateX(100%);
  transition: transform 0.2s ease-out;
  overflow-y: auto;
}

.slide-panel.open {
  transform: translateX(0);
}

.slide-panel-content {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.slide-panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.slide-panel-header h2 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-strong);
  margin: 0;
}

/* Panel form field */
.panel-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.panel-field label {
  font-size: 11px;
  color: var(--text-weak);
  text-transform: lowercase;
}

.panel-field p {
  font-size: 12px;
  color: var(--text);
  margin: 0;
  line-height: 1.4;
}

/* Primary action button (accent color) */
.btn-accent {
  padding: 8px 16px;
  background: var(--accent);
  border: 1px solid var(--accent);
  border-radius: 4px;
  font-family: var(--font);
  font-size: 12px;
  color: var(--bg);
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-accent:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-accent:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Danger button */
.btn-danger {
  background: transparent;
  border: 1px solid var(--danger, #e74c3c);
  color: var(--danger, #e74c3c);
  padding: 8px 16px;
  border-radius: 4px;
  font-family: var(--font);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-danger:hover:not(:disabled) {
  background: rgba(231, 76, 60, 0.1);
}
</style>

<style scoped>
.app {
  height: 100%;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.app.loaded {
  opacity: 1;
}

.window {
  height: 100%;
  display: flex;
  background: var(--bg);
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--border);
}

/* Sidebar */
.sidebar {
  width: 120px;
  flex-shrink: 0;
  background: var(--bg);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  user-select: none;
  -webkit-user-select: none;
}

.brand {
  padding: 20px 16px 24px;
}

.wordmark {
  font-family: var(--font);
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-strong);
  letter-spacing: -0.02em;
}

.nav {
  display: flex;
  flex-direction: column;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  background: transparent;
  border: none;
  border-left: 2px solid transparent;
  color: var(--text-weak);
  font-family: var(--font);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
  text-align: left;
  text-decoration: none;
}

.nav-item:hover {
  color: var(--text-strong);
  background: var(--bg-weak);
}

.nav-item.active {
  color: var(--accent);
  border-left-color: var(--accent);
}

.nav-marker {
  color: var(--icon);
  font-weight: 400;
}

.nav-item.active .nav-marker {
  color: var(--accent);
}

/* Content */
.content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  overflow: hidden;
  border-left: 1px solid var(--border);
}

/* Window controls */
.titlebar {
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 0 8px;
  flex-shrink: 0;
  user-select: none;
  -webkit-user-select: none;
}

.window-controls {
  display: flex;
  gap: 4px;
}

.control-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--icon);
  cursor: pointer;
  transition: all 0.15s ease;
}

.control-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.control-btn.close:hover {
  background: var(--recording);
  color: white;
}

.control-btn svg {
  width: 10px;
  height: 10px;
}
</style>
