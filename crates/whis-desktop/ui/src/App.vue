<script setup lang="ts" vapor>
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

import ShortcutView from './views/ShortcutView.vue';
import ApiKeyView from './views/ApiKeyView.vue';
import AboutView from './views/AboutView.vue';

interface Settings {
  shortcut: string;
  openai_api_key: string | null;
}

interface BackendInfo {
  backend: string;
  requires_restart: boolean;
  compositor: string;
  portal_version: number;
}

// Navigation
type Section = 'shortcut' | 'api-key' | 'about';
const activeSection = ref<Section>('shortcut');

// Settings state
const currentShortcut = ref("Ctrl+Shift+R");
const portalShortcut = ref<string | null>(null);
const apiKey = ref("");
const backendInfo = ref<BackendInfo | null>(null);
const loaded = ref(false);

// App info
const appVersion = "0.5.0";
const appRepo = "https://github.com/frankdierolf/whis";
const appSite = "https://whis.ink";

// Window decoration detection
const showCustomControls = ref(true);

async function loadSettings() {
  try {
    const settings = await invoke<Settings>('get_settings');
    currentShortcut.value = settings.shortcut;
    apiKey.value = settings.openai_api_key || '';
  } catch (e) {
    console.error("Failed to load settings:", e);
  }
}

// Window controls
async function minimizeWindow() {
  await getCurrentWindow().minimize();
}

async function closeWindow() {
  await getCurrentWindow().hide();
}

onMounted(async () => {
  // Get backend info first
  try {
    backendInfo.value = await invoke<BackendInfo>('get_shortcut_backend');
  } catch (e) {
    console.error('Failed to get backend info:', e);
  }

  // Always show custom controls - GTK titlebar fix enables dragging on Wayland
  showCustomControls.value = true;

  await loadSettings();

  // For portal backend, fetch actual binding
  if (backendInfo.value?.backend === 'PortalGlobalShortcuts') {
    try {
      portalShortcut.value = await invoke<string | null>('get_portal_shortcut');
    } catch (e) {
      console.error('Failed to get portal shortcut:', e);
    }
  }

  setTimeout(() => loaded.value = true, 50);
});
</script>

<template>
  <div class="app" :class="{ loaded }">
    <div class="window">
      <!-- Sidebar -->
      <aside class="sidebar" data-tauri-drag-region>
        <div class="brand" data-tauri-drag-region>
          <span class="wordmark">whis</span>
        </div>

        <nav class="nav">
          <button
            class="nav-item"
            :class="{ active: activeSection === 'shortcut' }"
            @click="activeSection = 'shortcut'"
          >
            <span class="nav-marker">{{ activeSection === 'shortcut' ? '>' : ' ' }}</span>
            <span>shortcut</span>
          </button>

          <button
            class="nav-item"
            :class="{ active: activeSection === 'api-key' }"
            @click="activeSection = 'api-key'"
          >
            <span class="nav-marker">{{ activeSection === 'api-key' ? '>' : ' ' }}</span>
            <span>api keys</span>
          </button>

          <button
            class="nav-item"
            :class="{ active: activeSection === 'about' }"
            @click="activeSection = 'about'"
          >
            <span class="nav-marker">{{ activeSection === 'about' ? '>' : ' ' }}</span>
            <span>about</span>
          </button>
        </nav>
      </aside>

      <!-- Content -->
      <main class="content">
        <!-- Title bar for dragging (only shown when native decorations are hidden) -->
        <div v-if="showCustomControls" class="titlebar" data-tauri-drag-region>
          <div class="window-controls">
            <button class="control-btn" @click="minimizeWindow" title="Minimize">
              <svg viewBox="0 0 10 10"><rect x="1" y="4.5" width="8" height="1" fill="currentColor"/></svg>
            </button>
            <button class="control-btn close" @click="closeWindow" title="Close">
              <svg viewBox="0 0 10 10"><path d="M1.5 1.5L8.5 8.5M8.5 1.5L1.5 8.5" stroke="currentColor" stroke-width="1.2"/></svg>
            </button>
          </div>
        </div>

        <!-- Views -->
        <ShortcutView
          v-if="activeSection === 'shortcut'"
          :backend-info="backendInfo"
          :current-shortcut="currentShortcut"
          :portal-shortcut="portalShortcut"
          :api-key="apiKey"
          @update:current-shortcut="currentShortcut = $event"
          @update:portal-shortcut="portalShortcut = $event"
        />

        <ApiKeyView
          v-if="activeSection === 'api-key'"
          v-model="apiKey"
          :current-shortcut="currentShortcut"
        />

        <AboutView
          v-if="activeSection === 'about'"
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
