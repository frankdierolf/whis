import type { AutotypeBackend, AutotypeToolStatus, BackendInfo, CliShortcutMode, OutputMethod, PostProcessor, Provider, Settings, ShortcutPathMismatch } from '../types'
import { invoke } from '@tauri-apps/api/core'
import { nextTick, reactive, readonly, watch } from 'vue'

// Defaults fetched from backend (single source of truth: whis-core/src/defaults.rs)
interface Defaults {
  provider: Provider
  post_processor: PostProcessor
  ollama_url: string
  ollama_model: string
  desktop_key: string
  vad_enabled: boolean
  vad_threshold: number
}

// Debounce utility with cancel support
function debounce<T extends (...args: unknown[]) => unknown>(fn: T, ms: number) {
  let timeoutId: ReturnType<typeof setTimeout> | null = null

  const debounced = (...args: Parameters<T>) => {
    if (timeoutId)
      clearTimeout(timeoutId)
    timeoutId = setTimeout(() => {
      timeoutId = null
      fn(...args)
    }, ms)
  }

  debounced.cancel = () => {
    if (timeoutId) {
      clearTimeout(timeoutId)
      timeoutId = null
    }
  }

  return debounced
}

// Cached defaults from backend (populated on init)
// These are fallback values until get_defaults() is called
let defaults: Defaults = {
  provider: 'deepgram',
  post_processor: 'openai',
  ollama_url: 'http://localhost:11434',
  ollama_model: 'qwen2.5:1.5b',
  desktop_key: 'Ctrl+Alt+W',
  vad_enabled: false,
  vad_threshold: 0.5,
}

// Get default settings using cached defaults
function getDefaultSettings(): Settings {
  return {
    transcription: {
      provider: defaults.provider,
      language: null,
      api_keys: {},
      local_models: {
        whisper_path: null,
        parakeet_path: null,
      },
    },
    post_processing: {
      enabled: false,
      processor: defaults.post_processor,
      prompt: null,
    },
    services: {
      ollama: {
        url: defaults.ollama_url,
        model: defaults.ollama_model,
        keep_alive: '5m',
      },
    },
    shortcuts: {
      cli_mode: 'system' as CliShortcutMode,
      cli_key: defaults.desktop_key,
      cli_push_to_talk: false,
      desktop_key: defaults.desktop_key,
    },
    ui: {
      clipboard_backend: 'auto',
      microphone_device: null,
      chunk_duration_secs: 90,
      output_method: 'clipboard' as OutputMethod,
      autotype_backend: 'auto' as AutotypeBackend,
      autotype_delay_ms: null,
      vad: {
        enabled: defaults.vad_enabled,
        threshold: defaults.vad_threshold,
      },
      active_preset: null,
      bubble: {
        enabled: false,
      },
      model_memory: {
        keep_model_loaded: true,
        unload_after_minutes: 10,
      },
    },
  }
}

// Internal mutable state
const state = reactive({
  // Settings (initialized with defaults, updated from backend)
  ...getDefaultSettings(),

  // Backend info
  backendInfo: null as BackendInfo | null,
  portalShortcut: null as string | null,
  portalBindError: null as string | null,
  rdevGrabError: null as string | null,
  isInInputGroup: false,
  systemShortcut: null as string | null, // GNOME custom shortcut (RdevGrab backend)
  shortcutPathMismatch: null as ShortcutPathMismatch | null, // Path mismatch warning

  // Autotype tool status
  autotypeToolStatus: null as AutotypeToolStatus | null,

  // Loading state
  loaded: false,

  // Platform capability state
  bubbleSupportsDrag: true,

  // Window visibility state (for smooth show/hide transitions)
  windowVisible: false,

  // Download state (not persisted to disk)
  whisperDownload: {
    active: false,
    model: null as string | null,
    progress: null as { downloaded: number, total: number } | null,
    error: null as string | null,
  },
  parakeetDownload: {
    active: false,
    model: null as string | null,
    progress: null as { downloaded: number, total: number } | null,
    error: null as string | null,
  },
})

// Build settings payload for save_settings command
function buildSettingsPayload(): Settings {
  return {
    transcription: state.transcription,
    post_processing: state.post_processing,
    services: state.services,
    shortcuts: state.shortcuts,
    ui: state.ui,
  }
}

// Debounced auto-save (500ms delay)
const debouncedSave = debounce(async () => {
  try {
    await invoke<{ needs_restart: boolean }>('save_settings', {
      settings: buildSettingsPayload(),
    })
  }
  catch (e) {
    console.error('Auto-save failed:', e)
  }
}, 500)

// Watch settings and auto-save on change
watch(
  () => [
    state.transcription,
    state.post_processing,
    state.services,
    state.shortcuts,
    state.ui,
  ],
  () => {
    if (state.loaded)
      debouncedSave()
  },
  { deep: true },
)

// Actions
async function load() {
  try {
    const settings = await invoke<Settings>('get_settings')
    // Deep copy nested settings, using cached defaults for fallbacks
    state.transcription = {
      provider: settings.transcription.provider || defaults.provider,
      language: settings.transcription.language,
      api_keys: settings.transcription.api_keys || {},
      local_models: {
        whisper_path: settings.transcription.local_models.whisper_path,
        parakeet_path: settings.transcription.local_models.parakeet_path,
      },
    }
    state.post_processing = {
      enabled: settings.post_processing.enabled ?? false,
      processor: settings.post_processing.processor || defaults.post_processor,
      prompt: settings.post_processing.prompt,
    }
    state.services = {
      ollama: {
        url: settings.services.ollama.url || defaults.ollama_url,
        model: settings.services.ollama.model || defaults.ollama_model,
        keep_alive: settings.services.ollama.keep_alive || '5m',
      },
    }
    state.shortcuts = {
      cli_mode: settings.shortcuts?.cli_mode || 'system',
      cli_key: settings.shortcuts?.cli_key || defaults.desktop_key,
      cli_push_to_talk: settings.shortcuts?.cli_push_to_talk ?? false,
      desktop_key: settings.shortcuts?.desktop_key || defaults.desktop_key,
    }
    state.ui = {
      clipboard_backend: settings.ui.clipboard_backend,
      microphone_device: settings.ui.microphone_device,
      chunk_duration_secs: Math.max(10, Math.min(300, settings.ui.chunk_duration_secs ?? 90)),
      output_method: settings.ui.output_method ?? 'clipboard',
      autotype_backend: settings.ui.autotype_backend ?? 'auto',
      autotype_delay_ms: settings.ui.autotype_delay_ms ?? null,
      vad: {
        enabled: settings.ui.vad.enabled ?? defaults.vad_enabled,
        threshold: settings.ui.vad.threshold ?? defaults.vad_threshold,
      },
      active_preset: settings.ui.active_preset,
      bubble: {
        enabled: settings.ui.bubble?.enabled ?? false,
      },
      model_memory: {
        keep_model_loaded: settings.ui.model_memory?.keep_model_loaded ?? true,
        unload_after_minutes: settings.ui.model_memory?.unload_after_minutes ?? 10,
      },
    }
  }
  catch (e) {
    console.error('Failed to load settings:', e)
  }
}

async function save(): Promise<boolean> {
  try {
    const result = await invoke<{ needs_restart: boolean }>('save_settings', {
      settings: buildSettingsPayload(),
    })
    return result.needs_restart
  }
  catch (e) {
    console.error('Failed to save settings:', e)
    throw e
  }
}

async function loadBackendInfo() {
  try {
    state.backendInfo = await invoke<BackendInfo>('shortcut_backend')

    // For portal backend, fetch actual binding and any errors
    if (state.backendInfo?.backend === 'PortalGlobalShortcuts') {
      state.portalShortcut = await invoke<string | null>('portal_shortcut')
      state.portalBindError = await invoke<string | null>('portal_bind_error')
      // Also read GNOME custom shortcut for display (Flatpak uses custom keybindings)
      state.systemShortcut = await invoke<string | null>('system_shortcut_from_dconf')
    }

    // For RdevGrab backend, fetch any grab errors and check input group
    if (state.backendInfo?.backend === 'RdevGrab') {
      state.rdevGrabError = await invoke<string | null>('rdev_grab_error')
      state.isInInputGroup = await invoke<boolean>('check_input_group_membership')

      // Try to read configured system shortcut from GNOME dconf
      state.systemShortcut = await invoke<string | null>('system_shortcut_from_dconf')

      // Check for shortcut path mismatch (configured vs current binary)
      state.shortcutPathMismatch = await invoke<ShortcutPathMismatch | null>('check_shortcut_path_mismatch')
    }
  }
  catch (e) {
    console.error('Failed to get backend info:', e)
  }
}

async function loadDefaults() {
  try {
    defaults = await invoke<Defaults>('get_defaults')
  }
  catch (e) {
    console.error('Failed to load defaults from backend:', e)
    // Keep using fallback defaults
  }
}

async function loadAutotypeToolStatus() {
  try {
    state.autotypeToolStatus = await invoke<AutotypeToolStatus>('get_autotype_tool_status_cmd')
  }
  catch (e) {
    console.error('Failed to load autotype tool status:', e)
  }
}

async function initialize() {
  // Load canonical defaults from backend first (single source of truth)
  await loadDefaults()
  await loadBackendInfo()
  await loadAutotypeToolStatus()
  await load()

  // Check if bubble drag is supported on this platform
  try {
    state.bubbleSupportsDrag = await invoke<boolean>('bubble_supports_drag')
  }
  catch {
    state.bubbleSupportsDrag = true // Default to true on error
  }

  // Query backend for active download state (survives window close/reopen)
  try {
    const activeDownload = await invoke<{
      model_name: string
      model_type: string
      downloaded: number
      total: number
    } | null>('get_active_download')

    if (activeDownload) {
      if (activeDownload.model_type === 'whisper') {
        state.whisperDownload.active = true
        state.whisperDownload.model = activeDownload.model_name
        state.whisperDownload.progress = {
          downloaded: activeDownload.downloaded,
          total: activeDownload.total,
        }
      }
      else if (activeDownload.model_type === 'parakeet') {
        state.parakeetDownload.active = true
        state.parakeetDownload.model = activeDownload.model_name
        state.parakeetDownload.progress = {
          downloaded: activeDownload.downloaded,
          total: activeDownload.total,
        }
      }
    }
  }
  catch {
    // Command not yet implemented or error - ignore
  }

  state.loaded = true
  await nextTick() // Ensure reactive updates propagate before watchers fire

  // Sync detected system shortcut to settings.json
  // Must be after state.loaded = true so watcher triggers auto-save
  if (state.backendInfo?.backend === 'RdevGrab' && state.systemShortcut) {
    if (state.systemShortcut !== state.shortcuts.desktop_key) {
      state.shortcuts.desktop_key = state.systemShortcut
      // Watcher automatically saves since state.loaded is true
    }
  }
}

async function waitForLoaded(): Promise<void> {
  if (state.loaded)
    return

  return new Promise((resolve) => {
    const unwatch = watch(
      () => state.loaded,
      (loaded) => {
        if (loaded) {
          unwatch()
          resolve()
        }
      },
      { immediate: true },
    )
  })
}

// Flush pending settings immediately (no debounce delay)
// Called before window close to ensure settings are persisted
async function flush(): Promise<void> {
  if (!state.loaded)
    return

  // Cancel any pending debounced save to avoid race condition
  debouncedSave.cancel()

  try {
    await invoke<{ needs_restart: boolean }>('save_settings', {
      settings: buildSettingsPayload(),
    })
  }
  catch (e) {
    console.error('Failed to flush settings:', e)
  }
}

// Setters for individual fields (for v-model binding)
function setProvider(value: Provider) {
  state.transcription.provider = value
}

function setLanguage(value: string | null) {
  state.transcription.language = value
}

function setApiKey(provider: string, key: string) {
  state.transcription.api_keys = { ...state.transcription.api_keys, [provider]: key }
}

function setWhisperModelPath(value: string | null) {
  state.transcription.local_models.whisper_path = value
}

function setParakeetModelPath(value: string | null) {
  state.transcription.local_models.parakeet_path = value
}

function setPostProcessor(value: PostProcessor) {
  state.post_processing.processor = value
}

function setOllamaUrl(value: string | null) {
  state.services.ollama.url = value
}

function setOllamaModel(value: string | null) {
  state.services.ollama.model = value
}

function setPostProcessingPrompt(value: string | null) {
  state.post_processing.prompt = value
}

function setDesktopKey(value: string) {
  state.shortcuts.desktop_key = value
}

function setPortalShortcut(value: string | null) {
  state.portalShortcut = value
}

function setMicrophoneDevice(value: string | null) {
  state.ui.microphone_device = value
}

function setBubbleEnabled(value: boolean) {
  state.ui.bubble.enabled = value
}

function setChunkDuration(value: number) {
  // Clamp to valid range (10-300 seconds)
  state.ui.chunk_duration_secs = Math.max(10, Math.min(300, value))
}

function setKeepModelLoaded(value: boolean) {
  state.ui.model_memory.keep_model_loaded = value
}

function setUnloadAfterMinutes(value: number) {
  state.ui.model_memory.unload_after_minutes = value
}

function setOllamaKeepAlive(value: string | null) {
  state.services.ollama.keep_alive = value
}

function setOutputMethod(value: OutputMethod) {
  state.ui.output_method = value
}

// Post-processing orchestration methods
function enablePostProcessing() {
  state.post_processing.enabled = true
}

function disablePostProcessing() {
  state.post_processing.enabled = false
  // Keep active_preset - user can re-enable without losing preset selection
}

async function applyPreset(name: string): Promise<void> {
  // Backend updates prompt, processor, and active_preset
  await invoke('apply_preset', { name })

  // Reload settings to get backend changes
  await load()

  // Enable post-processing when applying a preset
  state.post_processing.enabled = true
}

async function clearPreset(): Promise<void> {
  // Clear active preset in backend
  await invoke('set_active_preset', { name: null })

  // Reload settings
  await load()

  // Disable post-processing when clearing preset
  state.post_processing.enabled = false
}

// Window visibility state (for smooth show/hide transitions on Wayland)
function setWindowVisible(visible: boolean) {
  state.windowVisible = visible
}

// Download state management factory
type DownloadKey = 'whisperDownload' | 'parakeetDownload'

function createDownloadManager(key: DownloadKey) {
  return {
    start(model: string) {
      state[key].active = true
      state[key].model = model
      state[key].progress = null
      state[key].error = null
    },
    updateProgress(downloaded: number, total: number) {
      if (state[key].active) {
        state[key].progress = { downloaded, total }
      }
    },
    complete() {
      state[key].active = false
      state[key].progress = null
      state[key].model = null
    },
    fail(error: string) {
      state[key].active = false
      state[key].error = error
    },
  }
}

const whisperDownload = createDownloadManager('whisperDownload')
const parakeetDownload = createDownloadManager('parakeetDownload')

// Getter for default provider (used by SettingsView when switching modes)
function getDefaultProvider(): Provider {
  return defaults.provider
}

// Export reactive state and actions
export const settingsStore = {
  // Readonly state for reading (prevents accidental mutation)
  state: readonly(state),

  // Mutable state for v-model (use sparingly)
  mutableState: state,

  // Actions
  load,
  save,
  initialize,
  waitForLoaded,
  flush,
  getDefaultProvider,

  // Setters
  setProvider,
  setLanguage,
  setApiKey,
  setWhisperModelPath,
  setParakeetModelPath,
  setPostProcessor,
  setOllamaUrl,
  setOllamaModel,
  setPostProcessingPrompt,
  setDesktopKey,
  setPortalShortcut,
  setMicrophoneDevice,
  setBubbleEnabled,
  setChunkDuration,
  setKeepModelLoaded,
  setUnloadAfterMinutes,
  setOllamaKeepAlive,
  setOutputMethod,
  setWindowVisible,

  // Post-processing orchestration
  enablePostProcessing,
  disablePostProcessing,
  applyPreset,
  clearPreset,

  // Download state management
  startWhisperDownload: whisperDownload.start,
  updateWhisperDownloadProgress: whisperDownload.updateProgress,
  completeWhisperDownload: whisperDownload.complete,
  failWhisperDownload: whisperDownload.fail,
  startParakeetDownload: parakeetDownload.start,
  updateParakeetDownloadProgress: parakeetDownload.updateProgress,
  completeParakeetDownload: parakeetDownload.complete,
  failParakeetDownload: parakeetDownload.fail,
}
