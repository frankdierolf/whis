<script setup lang="ts">
import type { BubbleCloseEvent, CaptureDataEvent } from 'tauri-plugin-floating-bubble'
import { invoke } from '@tauri-apps/api/core'
import { hideBubble, onBubbleClick, onBubbleClose, onCaptureData, onCaptureStart, onCaptureStop, setBubbleState, signalFlushed, signalReady } from 'tauri-plugin-floating-bubble'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { headerStore } from './stores/header'
import { recordingStore } from './stores/recording'
import { settingsStore } from './stores/settings'

const route = useRoute()
const loaded = computed(() => settingsStore.state.loaded)

// Event cleanup functions
let unlistenBubbleClick: (() => void) | null = null
let unlistenBubbleClose: (() => void) | null = null
let unlistenCaptureStart: (() => void) | null = null
let unlistenCaptureData: (() => void) | null = null
let unlistenCaptureStop: (() => void) | null = null
let unlistenVisibility: (() => void) | null = null
const sidebarOpen = ref(false)

const navItems = [
  { path: '/', name: 'home', label: 'home' },
  { path: '/settings', name: 'settings', label: 'settings' },
  { path: '/presets', name: 'presets', label: 'presets' },
  { path: '/about', name: 'about', label: 'about' },
]

const currentPageLabel = computed(() => {
  const item = navItems.find(i => i.name === route.name)
  return item?.label ?? 'whis'
})

const headerAction = computed(() => headerStore.state.action)

function toggleSidebar() {
  sidebarOpen.value = !sidebarOpen.value
}

function closeSidebar() {
  sidebarOpen.value = false
}

/**
 * Determine the bubble state based on recording store state.
 */
function getBubbleState(): string {
  const state = recordingStore.state

  if (state.isRecording)
    return 'capturing'
  if (state.isTranscribing || state.isPostProcessing)
    return 'processing'
  return 'idle'
}

/**
 * Update bubble visual state (safe - catches errors if plugin unavailable).
 */
async function updateBubbleState() {
  const state = getBubbleState()
  try {
    await setBubbleState(state)
  }
  catch (error) {
    // Plugin may not be available
    console.error('[App.updateBubbleState] setBubbleState failed:', error)
  }
}

/**
 * Handle bubble close event (drag-to-close).
 */
async function handleBubbleClose(event: BubbleCloseEvent) {
  if (event.action === 'close') {
    try {
      await hideBubble()
      settingsStore.setFloatingBubbleEnabled(false)
    }
    catch (error) {
      console.error('[App.handleBubbleClose] hideBubble failed:', error)
    }
  }
}

/**
 * Handle app going to background - stop recording silently.
 * Mobile OS may restrict audio capture in background, causing broken recordings.
 */
function handleVisibilityChange() {
  if (document.hidden && recordingStore.state.isRecording) {
    recordingStore.stopRecording().catch((error) => {
      console.error('[App.handleVisibilityChange] Failed to stop recording:', error)
    })
  }
}

// Close sidebar on route change
watch(() => route.path, () => {
  closeSidebar()
})

onMounted(async () => {
  await settingsStore.initialize()
  await recordingStore.initialize()

  // Warm up HTTP client and cloud connections in background
  invoke('warmup_connections').catch(() => {})

  // Listen for bubble-click events from the floating bubble plugin
  try {
    unlistenBubbleClick = await onBubbleClick(async () => {
      await recordingStore.toggleRecording()
      // State will be updated by the watcher below
    })
  }
  catch {
    // Plugin may not be available on this platform
  }

  // Listen for bubble close events (drag-to-close)
  try {
    unlistenBubbleClose = await onBubbleClose(handleBubbleClose)
  }
  catch {
    // Plugin may not be available on this platform
  }

  // Listen for native capture events (background recording via floating bubble)
  try {
    // Capture started - initialize backend and signal ready
    unlistenCaptureStart = await onCaptureStart(async () => {
      try {
        await invoke('start_recording')
        signalReady()
      }
      catch (error) {
        console.error('[App] Failed to start recording:', error)
        signalReady() // Signal anyway to unblock native layer
      }
    })

    // Capture data - forward audio chunks to backend
    unlistenCaptureData = await onCaptureData(async (event: CaptureDataEvent) => {
      if (event.type === 'audio' && event.samples) {
        try {
          await invoke('send_audio_chunk', { samples: event.samples })
        }
        catch (error) {
          console.error('[App] Failed to send audio chunk:', error)
        }
      }
    })

    // Capture stopped - signal flushed and stop recording
    unlistenCaptureStop = await onCaptureStop(async () => {
      signalFlushed()
      try {
        await invoke('stop_recording')
        // Reset bubble state to idle after transcription completes
        await setBubbleState('idle')
      }
      catch (error) {
        console.error('[App] Failed to stop recording:', error)
        // Also reset to idle on error to avoid stuck state
        try {
          await setBubbleState('idle')
        }
        catch {}
      }
    })
  }
  catch {
    // Plugin may not be available on this platform
  }

  // Stop recording when app goes to background (mobile-specific)
  document.addEventListener('visibilitychange', handleVisibilityChange)
  unlistenVisibility = () => {
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  }

  // Watch all recording states to update bubble appearance
  watch(
    () => [
      recordingStore.state.isRecording,
      recordingStore.state.isTranscribing,
      recordingStore.state.isPostProcessing,
    ],
    () => {
      updateBubbleState()
    },
    { immediate: true },
  )
})

onUnmounted(() => {
  unlistenBubbleClick?.()
  unlistenBubbleClose?.()
  unlistenCaptureStart?.()
  unlistenCaptureData?.()
  unlistenCaptureStop?.()
  unlistenVisibility?.()
  recordingStore.cleanup()
})
</script>

<template>
  <div class="app" :class="{ loaded, 'sidebar-open': sidebarOpen }">
    <!-- Mobile Header -->
    <header class="mobile-header">
      <button
        class="menu-toggle"
        :aria-expanded="sidebarOpen"
        aria-label="Toggle menu"
        @click="toggleSidebar"
      >
        <span>{{ sidebarOpen ? '[x]' : '[=]' }}</span>
      </button>
      <span class="mobile-brand">{{ currentPageLabel }}</span>
      <button
        v-if="headerAction"
        class="header-action"
        :aria-label="headerAction.ariaLabel"
        @click="headerAction.onClick"
      >
        {{ headerAction.label }}
      </button>
    </header>

    <!-- Backdrop -->
    <div class="backdrop" @click="closeSidebar" />

    <!-- Sidebar Drawer -->
    <aside class="sidebar">
      <nav class="nav">
        <RouterLink
          v-for="item in navItems"
          :key="item.name"
          :to="item.path"
          class="nav-item"
          :class="{ active: route.name === item.name }"
          @click="closeSidebar"
        >
          <span class="nav-marker" aria-hidden="true">{{
            route.name === item.name ? '>' : ' '
          }}</span>
          <span>{{ item.label }}</span>
        </RouterLink>
      </nav>
    </aside>

    <!-- Content -->
    <main class="content">
      <router-view />
    </main>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  background: var(--bg);
  opacity: 0;
  transition: opacity 0.15s ease;
}

.app.loaded {
  opacity: 1;
}

/* Mobile Header - always visible on mobile app */
.mobile-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: calc(48px + env(safe-area-inset-top, 0px));
  padding-top: env(safe-area-inset-top, 0px);
  background: var(--bg);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  padding-left: 16px;
  padding-right: 16px;
  gap: 12px;
  z-index: 101;
}

.menu-toggle {
  all: unset;
  font-family: var(--font);
  font-size: 14px;
  color: var(--text);
  cursor: pointer;
  padding: 8px 4px;
  min-width: var(--touch-target-min);
  min-height: var(--touch-target-min);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.15s ease;
}

.menu-toggle:active {
  color: var(--accent);
}

.mobile-brand {
  font-family: var(--font);
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-strong);
  letter-spacing: -0.03em;
  flex: 1;
}

.header-action {
  all: unset;
  font-family: var(--font);
  font-size: 14px;
  color: var(--text-weak);
  cursor: pointer;
  padding: 8px 4px;
  min-width: var(--touch-target-min);
  min-height: var(--touch-target-min);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.15s ease;
}

.header-action:active {
  color: var(--accent);
}

/* Backdrop */
.backdrop {
  position: fixed;
  inset: 0;
  top: calc(48px + env(safe-area-inset-top, 0px));
  background: rgba(0, 0, 0, 0.6);
  z-index: 99;
  opacity: 0;
  transition: opacity 0.2s ease;
  pointer-events: none;
}

.sidebar-open .backdrop {
  opacity: 1;
  pointer-events: auto;
}

/* Sidebar Drawer */
.sidebar {
  position: fixed;
  top: calc(48px + env(safe-area-inset-top, 0px));
  left: 0;
  bottom: 0;
  width: 220px;
  background: var(--bg);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  transform: translateX(-100%);
  transition: transform 0.25s ease;
  z-index: 100;
  padding-bottom: env(safe-area-inset-bottom, 0px);
}

.sidebar-open .sidebar {
  transform: translateX(0);
}

.nav {
  display: flex;
  flex-direction: column;
  flex: 1;
  padding-top: 8px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 20px;
  border-left: 2px solid transparent;
  color: var(--text-weak);
  font-family: var(--font);
  font-size: 14px;
  min-height: var(--touch-target-min);
  cursor: pointer;
  text-decoration: none;
  transition: all 0.15s ease;
}

.nav-item:active {
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
  width: 0.75em;
}

.nav-item.active .nav-marker {
  color: var(--accent);
}

/* Content */
.content {
  height: calc(100vh - 48px - env(safe-area-inset-top, 0px));
  margin-top: calc(48px + env(safe-area-inset-top, 0px));
  overflow-y: auto;
}
</style>
