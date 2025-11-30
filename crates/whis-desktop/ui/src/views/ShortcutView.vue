<script setup lang="ts" vapor>
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';

interface BackendInfo {
  backend: string;
  requires_restart: boolean;
  compositor: string;
  portal_version: number;
}

interface SaveResult {
  needs_restart: boolean;
}

const props = defineProps<{
  backendInfo: BackendInfo | null;
  currentShortcut: string;
  portalShortcut: string | null;
  apiKey: string;
}>();

const emit = defineEmits<{
  'update:currentShortcut': [value: string];
  'update:portalShortcut': [value: string | null];
}>();

const isRecording = ref(false);
const status = ref("");
const needsRestart = ref(false);
const copied = ref(false);
const resetCommand = "dconf reset -f /org/gnome/settings-daemon/global-shortcuts/";

// Split shortcut into individual keys for display
const shortcutKeys = computed(() => {
  const binding = props.backendInfo?.backend === 'PortalGlobalShortcuts'
    && props.portalShortcut
    ? props.portalShortcut
    : props.currentShortcut;

  if (binding === "Press keys...") {
    return ["..."];
  }
  return binding.split('+');
});

async function copyResetCommand() {
  await navigator.clipboard.writeText(resetCommand);
  copied.value = true;
  setTimeout(() => copied.value = false, 1500);
}

async function saveShortcut() {
  try {
    const result = await invoke<SaveResult>('save_settings', {
      settings: {
        shortcut: props.currentShortcut,
        openai_api_key: props.apiKey || null
      }
    });
    if (result.needs_restart) {
      needsRestart.value = true;
      status.value = "";
    } else {
      status.value = "Saved";
      setTimeout(() => status.value = "", 2000);
    }
  } catch (e) {
    status.value = "Failed to save: " + e;
  }
}

async function openConfigureDialog() {
  try {
    status.value = "Opening...";
    const newBinding = await invoke<string | null>('configure_shortcut');
    if (newBinding) {
      emit('update:portalShortcut', newBinding);
    }
    status.value = "";
  } catch (e) {
    status.value = "Failed: " + e;
  }
}

async function restartApp() {
  await relaunch();
}

function handleKeyDown(e: KeyboardEvent) {
  if (!isRecording.value) return;
  e.preventDefault();

  const keys = [];
  if (e.ctrlKey) keys.push('Ctrl');
  if (e.shiftKey) keys.push('Shift');
  if (e.altKey) keys.push('Alt');
  if (e.metaKey) keys.push('Super');

  const key = e.key.toUpperCase();
  if (!['CONTROL', 'SHIFT', 'ALT', 'META'].includes(key)) {
    keys.push(key);
  }

  if (keys.length > 0) {
    emit('update:currentShortcut', keys.join('+'));
  }
}

function startRecording() {
  isRecording.value = true;
  emit('update:currentShortcut', "Press keys...");
}

function stopRecording() {
  isRecording.value = false;
  // Parent will reload settings if needed
}
</script>

<template>
  <section class="section">
    <header class="section-header">
      <h1>Global Shortcut</h1>
      <p>Toggle recording from anywhere</p>
    </header>

    <div class="section-content">
      <!-- Portal backend (Wayland) -->
      <template v-if="backendInfo?.backend === 'PortalGlobalShortcuts'">

        <!-- Portal v2+: Configure button -->
        <template v-if="backendInfo.portal_version >= 2">
          <p class="hint">Managed by your desktop environment.</p>
          <button @click="openConfigureDialog" class="btn">
            Configure
          </button>
        </template>

        <!-- Portal v1: Read-only -->
        <template v-else>
          <div class="notice">
            <span class="notice-marker">[!]</span>
            <p>Shortcuts are locked after first launch on GNOME {{ backendInfo.compositor.includes('47') ? '47' : '46' }}.</p>
          </div>

          <div class="field">
            <label>current binding</label>
            <div class="keys">
              <span v-for="(key, index) in shortcutKeys" :key="index" class="key">{{ key }}</span>
            </div>
          </div>

          <div class="reset-info">
            <label>to reset</label>
            <div class="command" :class="{ copied }" @click="copyResetCommand">
              <code>{{ resetCommand }}</code>
              <button class="copy-btn" type="button">
                <svg v-if="!copied" class="icon-copy" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                </svg>
                <svg v-else class="icon-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
              </button>
            </div>
            <p class="hint">Then restart Whis. GNOME 48+ allows direct configuration.</p>
          </div>
        </template>
      </template>

      <!-- CLI Fallback (Wayland without portal support) -->
      <template v-else-if="backendInfo?.backend === 'CLIFallback'">
        <div class="notice warning">
          <span class="notice-marker">[!]</span>
          <p>Global shortcuts require manual configuration on {{ backendInfo.compositor }}.</p>
        </div>

        <div class="instructions">
          <label>setup instructions</label>

          <!-- GNOME -->
          <template v-if="backendInfo.compositor.toLowerCase().includes('gnome')">
            <ol class="steps">
              <li>Open <strong>Settings</strong> → <strong>Keyboard</strong> → <strong>Custom Shortcuts</strong></li>
              <li>Add a new shortcut with these values:</li>
            </ol>
            <div class="command-block">
              <div class="command-row">
                <span class="command-label">Name:</span>
                <code>Whis Toggle Recording</code>
              </div>
              <div class="command-row">
                <span class="command-label">Command:</span>
                <code>flatpak run ink.whis.desktop --toggle</code>
              </div>
              <div class="command-row">
                <span class="command-label">Shortcut:</span>
                <code>{{ currentShortcut }}</code>
              </div>
            </div>
          </template>

          <!-- KDE/Plasma -->
          <template v-else-if="backendInfo.compositor.toLowerCase().includes('kde') || backendInfo.compositor.toLowerCase().includes('plasma')">
            <ol class="steps">
              <li>Open <strong>System Settings</strong> → <strong>Shortcuts</strong> → <strong>Custom Shortcuts</strong></li>
              <li>Add a new shortcut:</li>
            </ol>
            <div class="command-block">
              <div class="command-row">
                <span class="command-label">Command:</span>
                <code>flatpak run ink.whis.desktop --toggle</code>
              </div>
            </div>
          </template>

          <!-- Sway -->
          <template v-else-if="backendInfo.compositor.toLowerCase().includes('sway')">
            <p class="hint">Add to <code>~/.config/sway/config</code>:</p>
            <div class="command">
              <code>bindsym {{ currentShortcut.toLowerCase() }} exec flatpak run ink.whis.desktop --toggle</code>
            </div>
          </template>

          <!-- Hyprland -->
          <template v-else-if="backendInfo.compositor.toLowerCase().includes('hyprland')">
            <p class="hint">Add to <code>~/.config/hypr/hyprland.conf</code>:</p>
            <div class="command">
              <code>bind = {{ currentShortcut.replace(/\+/g, ', ') }}, exec, flatpak run ink.whis.desktop --toggle</code>
            </div>
          </template>

          <!-- Generic -->
          <template v-else>
            <p class="hint">Configure your compositor to run:</p>
            <div class="command">
              <code>flatpak run ink.whis.desktop --toggle</code>
            </div>
          </template>
        </div>
      </template>

      <!-- Tauri plugin (X11/macOS/Windows) -->
      <template v-else>
        <div class="field">
          <label>press to record</label>
          <div
            class="shortcut-input"
            :class="{ recording: isRecording }"
            @click="startRecording"
            @blur="stopRecording"
            @keydown="handleKeyDown"
            tabindex="0"
          >
            <div class="keys">
              <span
                v-for="(key, index) in shortcutKeys"
                :key="index"
                class="key"
                :class="{ placeholder: key === '...' }"
              >{{ key }}</span>
            </div>
            <span v-if="isRecording" class="recording-dot"></span>
          </div>
        </div>

        <button @click="saveShortcut" class="btn" :disabled="isRecording">
          Save
        </button>

        <!-- Restart banner -->
        <div v-if="needsRestart" class="restart-banner">
          <span>[*] Restart required</span>
          <button @click="restartApp" class="btn-link">Restart now</button>
        </div>
      </template>

      <!-- Status -->
      <div class="status" :class="{ visible: status }">{{ status }}</div>
    </div>
  </section>
</template>

<style scoped>
/* Keys display */
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

.key.placeholder {
  color: var(--text-weak);
}

/* Shortcut input */
.shortcut-input {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.shortcut-input:hover {
  border-color: var(--text-weak);
}

.shortcut-input:focus {
  outline: none;
  border-color: var(--accent);
}

.shortcut-input.recording {
  border-color: var(--recording);
}

.recording-dot {
  width: 6px;
  height: 6px;
  background: var(--recording);
  border-radius: 50%;
  animation: pulse 1s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

/* Command block */
.command {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  padding: 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  cursor: pointer;
  transition: border-color 0.15s ease;
}

.command:hover {
  border-color: var(--text-weak);
}

.command.copied {
  border-color: var(--accent);
}

.command code {
  flex: 1;
  min-width: 0;
  font-family: var(--font);
  font-size: 11px;
  color: var(--text);
  word-break: break-all;
  line-height: 1.5;
}

.copy-btn {
  all: unset;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  color: var(--icon);
  cursor: pointer;
  transition: color 0.15s ease;
}

.copy-btn:hover {
  color: var(--text-strong);
}

.command.copied .copy-btn {
  color: var(--accent);
}

.copy-btn svg {
  width: 14px;
  height: 14px;
}

/* Reset info */
.reset-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.reset-info label {
  font-size: 11px;
  text-transform: lowercase;
  color: var(--text-weak);
}

/* Restart banner */
.restart-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-size: 12px;
  color: var(--text);
}

/* CLI Fallback instructions */
.notice.warning {
  border-color: var(--warning, #f59e0b);
}

.notice.warning .notice-marker {
  color: var(--warning, #f59e0b);
}

.instructions {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.instructions label {
  font-size: 11px;
  text-transform: lowercase;
  color: var(--text-weak);
}

.steps {
  margin: 0;
  padding-left: 20px;
  font-size: 12px;
  color: var(--text);
  line-height: 1.6;
}

.steps li {
  margin-bottom: 4px;
}

.steps strong {
  color: var(--text-strong);
}

.command-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
}

.command-row {
  display: flex;
  gap: 8px;
  align-items: baseline;
}

.command-label {
  font-size: 11px;
  color: var(--text-weak);
  min-width: 70px;
}

.command-block code {
  font-family: var(--font);
  font-size: 11px;
  color: var(--accent);
}
</style>
