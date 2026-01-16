<script setup lang="ts">
import type { PostProcessor, Provider, SelectOption, TranscriptionMethod } from '../types'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import * as bubble from 'tauri-plugin-floating-bubble'
import { computed, onMounted, ref } from 'vue'
import AppInput from '../components/AppInput.vue'
import AppSelect from '../components/AppSelect.vue'
import ToggleSwitch from '../components/ToggleSwitch.vue'
import { presetsStore } from '../stores/presets'
import { settingsStore } from '../stores/settings'

// Floating bubble permission state
const hasOverlayPermission = ref(false)
const hasMicrophonePermission = ref(false)
const bubbleError = ref<string | null>(null)

// Provider options (loaded from backend, ordered by recommendation from whis-core)
const providerOptions = ref<SelectOption[]>([])

// Language options
const languageOptions: SelectOption[] = [
  { value: null, label: 'Auto-detect' },
  { value: 'en', label: 'English' },
  { value: 'de', label: 'German' },
  { value: 'fr', label: 'French' },
  { value: 'es', label: 'Spanish' },
  { value: 'it', label: 'Italian' },
  { value: 'pt', label: 'Portuguese' },
  { value: 'nl', label: 'Dutch' },
  { value: 'pl', label: 'Polish' },
  { value: 'ja', label: 'Japanese' },
  { value: 'zh', label: 'Chinese' },
  { value: 'ko', label: 'Korean' },
]

// Post-processor options
const postProcessorOptions: SelectOption[] = [
  { value: 'none', label: 'Disabled' },
  { value: 'openai', label: 'OpenAI' },
  { value: 'mistral', label: 'Mistral' },
]

// Computed bindings to store
const provider = computed({
  get: () => settingsStore.state.provider,
  set: val => settingsStore.setProvider(val),
})

const language = computed({
  get: () => settingsStore.state.language,
  set: val => settingsStore.setLanguage(val),
})

const openaiApiKey = computed({
  get: () => settingsStore.state.openai_api_key ?? '',
  set: val => settingsStore.setOpenaiApiKey(val || null),
})

const mistralApiKey = computed({
  get: () => settingsStore.state.mistral_api_key ?? '',
  set: val => settingsStore.setMistralApiKey(val || null),
})

const groqApiKey = computed({
  get: () => settingsStore.state.groq_api_key ?? '',
  set: val => settingsStore.setGroqApiKey(val || null),
})

const deepgramApiKey = computed({
  get: () => settingsStore.state.deepgram_api_key ?? '',
  set: val => settingsStore.setDeepgramApiKey(val || null),
})

const elevenlabsApiKey = computed({
  get: () => settingsStore.state.elevenlabs_api_key ?? '',
  set: val => settingsStore.setElevenlabsApiKey(val || null),
})

// OpenAI streaming method
const openaiMethod = computed<TranscriptionMethod>({
  get: () => provider.value === 'openai-realtime' ? 'streaming' : 'standard',
  set: (val) => {
    const newProvider = val === 'streaming' ? 'openai-realtime' : 'openai'
    settingsStore.setProvider(newProvider)
  },
})

// Show streaming toggle only for OpenAI
const showStreamingToggle = computed(() =>
  provider.value === 'openai' || provider.value === 'openai-realtime',
)

// Streaming toggle state (convert TranscriptionMethod to boolean)
const isStreamingEnabled = computed({
  get: () => openaiMethod.value === 'streaming',
  set: (value: boolean) => {
    openaiMethod.value = value ? 'streaming' : 'standard'
  },
})

// Deepgram streaming method
const deepgramMethod = computed<TranscriptionMethod>({
  get: () => provider.value === 'deepgram-realtime' ? 'streaming' : 'standard',
  set: (val) => {
    const newProvider = val === 'streaming' ? 'deepgram-realtime' : 'deepgram'
    settingsStore.setProvider(newProvider)
  },
})

// Show streaming toggle for Deepgram
const showDeepgramStreamingToggle = computed(() =>
  provider.value === 'deepgram' || provider.value === 'deepgram-realtime',
)

// Deepgram streaming toggle state
const isDeepgramStreamingEnabled = computed({
  get: () => deepgramMethod.value === 'streaming',
  set: (value: boolean) => {
    deepgramMethod.value = value ? 'streaming' : 'standard'
  },
})

// Normalize provider for display (realtime variants show as base provider)
const displayProvider = computed<Provider>(() => {
  switch (provider.value) {
    case 'openai-realtime':
      return 'openai'
    case 'deepgram-realtime':
      return 'deepgram'
    default:
      return provider.value
  }
})

// Post-processor binding
const postProcessor = computed({
  get: () => settingsStore.state.post_processor,
  set: val => settingsStore.setPostProcessor(val as PostProcessor),
})

// Check if post-processor is missing its required API key
const postProcessorMissingKey = computed(() => {
  if (postProcessor.value === 'openai' && !openaiApiKey.value)
    return 'OpenAI'
  if (postProcessor.value === 'mistral' && !mistralApiKey.value)
    return 'Mistral'
  return null
})

// Active preset (read-only display)
const activePresetName = computed(() => {
  const active = presetsStore.state.presets.find(p => p.is_active)
  return active?.name ?? 'None'
})

// Floating bubble toggle
const floatingBubbleEnabled = computed({
  get: () => settingsStore.state.floating_bubble_enabled,
  set: val => handleFloatingBubbleToggle(val),
})

// Check overlay permission and sync bubble state on mount
onMounted(async () => {
  presetsStore.loadPresets()
  await checkOverlayPermission()
  await checkMicrophonePermission()

  // Load cloud providers from backend (ordered by recommendation from whis-core)
  try {
    const providers = await invoke<{ value: string, label: string }[]>('get_cloud_providers')
    providerOptions.value = providers
  }
  catch (error) {
    console.error('Failed to load cloud providers:', error)
  }

  // If bubble was enabled but permission was revoked, disable it
  if (settingsStore.state.floating_bubble_enabled && (!hasOverlayPermission.value || !hasMicrophonePermission.value)) {
    settingsStore.setFloatingBubbleEnabled(false)
  }

  // Restore bubble state if enabled and both permissions granted
  if (settingsStore.state.floating_bubble_enabled && hasOverlayPermission.value && hasMicrophonePermission.value) {
    await showBubble()
  }
})

// Check if overlay permission is granted
async function checkOverlayPermission() {
  try {
    const result = await bubble.hasOverlayPermission()
    hasOverlayPermission.value = result.granted
  }
  catch (e) {
    console.error('Failed to check overlay permission:', e)
    hasOverlayPermission.value = false
  }
}

// Check if microphone permission is granted
async function checkMicrophonePermission() {
  try {
    const result = await bubble.hasMicrophonePermission()
    hasMicrophonePermission.value = result.granted
  }
  catch (e) {
    console.error('Failed to check microphone permission:', e)
    hasMicrophonePermission.value = false
  }
}

// Handle floating bubble toggle
async function handleFloatingBubbleToggle(enabled: boolean) {
  bubbleError.value = null

  if (!enabled) {
    await hideBubble()
    settingsStore.setFloatingBubbleEnabled(false)
    return
  }

  // Check overlay permission first
  if (!hasOverlayPermission.value) {
    try {
      await bubble.requestOverlayPermission()
      bubbleError.value = 'Please enable "Display over other apps" permission and try again'
    }
    catch (e) {
      bubbleError.value = `Failed to request permission: ${e}`
    }
    return
  }

  // Check microphone permission (required for foreground service with mic type on Android 14+)
  if (!hasMicrophonePermission.value) {
    try {
      await bubble.requestMicrophonePermission()
      // Re-check after dialog - user may have granted it
      await checkMicrophonePermission()
      if (!hasMicrophonePermission.value) {
        bubbleError.value = 'Microphone permission is required for background recording. Please grant the permission and try again.'
        return
      }
    }
    catch (e) {
      bubbleError.value = `Failed to request microphone permission: ${e}`
      return
    }
  }

  await showBubble()
  settingsStore.setFloatingBubbleEnabled(true)
}

// Whis bubble configuration
const BUBBLE_CONFIG = {
  size: 60,
  startX: 0,
  startY: 200,
  iconResourceName: 'ic_whis_logo_idle',
  background: '#1C1C1C',
  states: {
    idle: { iconResourceName: 'ic_whis_logo_idle' },
    recording: { iconResourceName: 'ic_whis_logo_recording' },
    processing: { iconResourceName: 'ic_whis_logo_processing' },
  },
}

// Show the floating bubble with Whis-specific configuration
async function showBubble() {
  try {
    await bubble.showBubble(BUBBLE_CONFIG)
  }
  catch (e) {
    console.error('Failed to show bubble:', e)
    bubbleError.value = `Failed to show bubble: ${e}`
    settingsStore.setFloatingBubbleEnabled(false)
  }
}

// Hide the floating bubble
async function hideBubble() {
  try {
    await bubble.hideBubble()
  }
  catch (e) {
    console.error('Failed to hide bubble:', e)
  }
}

// Re-check permissions when app regains focus (user may have granted permission)
if (typeof document !== 'undefined') {
  document.addEventListener('visibilitychange', async () => {
    if (document.visibilityState === 'visible') {
      await checkOverlayPermission()
      await checkMicrophonePermission()
    }
  })
}
</script>

<template>
  <div class="settings-view">
    <main class="settings-content">
      <!-- Provider Section -->
      <div class="settings-section">
        <p class="section-label">
          provider
        </p>

        <!-- Provider -->
        <div class="field">
          <label>service</label>
          <AppSelect
            :model-value="displayProvider"
            :options="providerOptions"
            aria-label="Select provider"
            @update:model-value="(val) => provider = val as Provider"
          />
        </div>

        <!-- OpenAI API Key -->
        <div v-if="provider === 'openai' || provider === 'openai-realtime'" class="field">
          <label>api key</label>
          <AppInput
            v-model="openaiApiKey"
            type="password"
            placeholder="sk-..."
          />
          <span class="hint">
            Get your key at <span class="link" @click="openUrl('https://platform.openai.com/api-keys')">platform.openai.com</span>
          </span>
        </div>

        <!-- Streaming Toggle (OpenAI only) -->
        <div v-if="showStreamingToggle" class="field streaming-field">
          <label>streaming mode</label>
          <div class="field-row">
            <ToggleSwitch v-model="isStreamingEnabled" />
            <span class="method-description">
              {{ openaiMethod === 'streaming' ? 'Real-time' : 'Standard' }}
            </span>
          </div>
        </div>

        <!-- Mistral API Key -->
        <div v-if="provider === 'mistral'" class="field">
          <label>api key</label>
          <AppInput
            v-model="mistralApiKey"
            type="password"
            placeholder="Enter API key"
          />
          <span class="hint">
            Get your key at <span class="link" @click="openUrl('https://console.mistral.ai/api-keys')">console.mistral.ai</span>
          </span>
        </div>

        <!-- Groq API Key -->
        <div v-if="provider === 'groq'" class="field">
          <label>api key</label>
          <AppInput
            v-model="groqApiKey"
            type="password"
            placeholder="gsk_..."
          />
          <span class="hint">
            Get your key at <span class="link" @click="openUrl('https://console.groq.com/keys')">console.groq.com</span>
          </span>
        </div>

        <!-- Deepgram API Key -->
        <div v-if="provider === 'deepgram' || provider === 'deepgram-realtime'" class="field">
          <label>api key</label>
          <AppInput
            v-model="deepgramApiKey"
            type="password"
            placeholder="Enter API key"
          />
          <span class="hint">
            Get your key at <span class="link" @click="openUrl('https://console.deepgram.com')">console.deepgram.com</span>
          </span>
        </div>

        <!-- Streaming Toggle (Deepgram) -->
        <div v-if="showDeepgramStreamingToggle" class="field streaming-field">
          <label>streaming mode</label>
          <div class="field-row">
            <ToggleSwitch v-model="isDeepgramStreamingEnabled" />
            <span class="method-description">
              {{ deepgramMethod === 'streaming' ? 'Real-time' : 'Standard' }}
            </span>
          </div>
        </div>

        <!-- ElevenLabs API Key -->
        <div v-if="provider === 'elevenlabs'" class="field">
          <label>api key</label>
          <AppInput
            v-model="elevenlabsApiKey"
            type="password"
            placeholder="Enter API key"
          />
          <span class="hint">
            Get your key at <span class="link" @click="openUrl('https://elevenlabs.io/app/settings/api-keys')">elevenlabs.io</span>
          </span>
        </div>
      </div>

      <!-- Transcription Section -->
      <div class="settings-section">
        <p class="section-label">
          transcription
        </p>

        <!-- Language -->
        <div class="field">
          <label>language</label>
          <AppSelect
            v-model="language"
            :options="languageOptions"
            aria-label="Select language"
          />
        </div>
      </div>

      <!-- Post-Processing Section -->
      <div class="settings-section">
        <p class="section-label">
          post-processing
        </p>

        <!-- Post-Processing -->
        <div class="field">
          <label>processor</label>
          <AppSelect
            v-model="postProcessor"
            :options="postProcessorOptions"
            aria-label="Select post-processor"
          />
          <div v-if="postProcessorMissingKey" class="api-key-warning">
            <span class="warning-marker">[!]</span>
            <span>Add your {{ postProcessorMissingKey }} API key in Provider</span>
          </div>
        </div>

        <!-- Active Preset (only shown when post-processing is enabled) -->
        <div v-if="postProcessor !== 'none'" class="field">
          <label>active preset</label>
          <div class="field-value">
            {{ activePresetName }}
          </div>
        </div>
      </div>

      <!-- Floating Bubble Section -->
      <div class="settings-section">
        <p class="section-label">
          floating bubble
        </p>

        <div class="field">
          <label>show bubble</label>
          <div class="field-row">
            <ToggleSwitch v-model="floatingBubbleEnabled" />
            <span class="method-description">
              {{ floatingBubbleEnabled ? 'Enabled' : 'Disabled' }}
            </span>
          </div>
          <span v-if="!hasOverlayPermission && floatingBubbleEnabled" class="warning-hint">
            Overlay permission required. Tap toggle to request.
          </span>
          <span v-if="hasOverlayPermission && !hasMicrophonePermission && floatingBubbleEnabled" class="warning-hint">
            Microphone permission required. Tap toggle to request.
          </span>
          <span v-if="bubbleError" class="error-hint">
            {{ bubbleError }}
          </span>
        </div>
      </div>

      <!-- Auto-save notice -->
      <div class="auto-save-notice">
        <span class="notice-marker">[*]</span>
        <span>Settings are saved automatically</span>
      </div>
    </main>
  </div>
</template>

<style scoped>
.settings-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0; /* Required for child overflow-y to work in flex container */
  overflow: hidden;
}

.settings-content {
  flex: 1;
  padding: 20px;
  padding-bottom: max(80px, calc(env(safe-area-inset-bottom) + 40px));
  display: flex;
  flex-direction: column;
  gap: 28px;
  overflow-y: auto;
  min-height: 0; /* Required for overflow-y to work in flex container */
}

/* Settings Sections - matches desktop layout */
.settings-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.section-label {
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-weak);
  letter-spacing: 0.05em;
  margin: 0;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border);
}

/* Field row for toggle + text */
.field-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.method-description {
  font-size: 14px;
  color: var(--text);
}

/* Read-only field value (for preset display) */
.field-value {
  padding: 12px 14px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-size: 14px;
  color: var(--text);
}

.auto-save-notice {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: auto;
  padding-top: 24px;
  font-size: 13px;
  color: var(--text-weak);
}

.notice-marker {
  color: var(--accent);
}

.link {
  color: var(--text-strong);
  text-decoration: underline;
  text-underline-offset: 2px;
  cursor: pointer;
}

.link:active {
  color: var(--accent);
}

.hint {
  display: block;
  font-size: 12px;
  color: var(--text-weak);
  margin-top: 8px;
  line-height: 1.5;
}

.api-key-warning {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  font-size: 13px;
  color: var(--text-weak);
}

.warning-marker {
  color: var(--accent);
}

.warning-hint {
  display: block;
  font-size: 12px;
  color: #f59e0b;
  margin-top: 8px;
  line-height: 1.5;
}

.error-hint {
  display: block;
  font-size: 12px;
  color: #ef4444;
  margin-top: 8px;
  line-height: 1.5;
}
</style>
