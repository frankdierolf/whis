// Generic select option for dropdowns
export interface SelectOption<T = string | null> {
  value: T
  label: string
  disabled?: boolean
}

// Transcription providers
export type Provider
  = | 'openai'
    | 'openai-realtime'
    | 'mistral'
    | 'groq'
    | 'deepgram'
    | 'deepgram-realtime'
    | 'elevenlabs'
    | 'local-whisper'
    | 'local-parakeet'

// OpenAI transcription method
export type TranscriptionMethod = 'standard' | 'streaming'

// Text post-processing providers
export type PostProcessor = 'none' | 'openai' | 'mistral' | 'ollama'

// CLI shortcut mode
export type CliShortcutMode = 'system' | 'direct'

// All settings from the backend (nested structure)
export interface Settings {
  transcription: {
    provider: Provider
    language: string | null
    api_keys: Record<string, string>
    local_models: {
      whisper_path: string | null
      parakeet_path: string | null
    }
  }
  post_processing: {
    enabled: boolean
    processor: PostProcessor
    prompt: string | null
  }
  services: {
    ollama: {
      url: string | null
      model: string | null
      keep_alive: string | null
    }
  }
  shortcuts: {
    cli_mode: CliShortcutMode
    cli_key: string
    cli_push_to_talk: boolean
    desktop_key: string
  }
  ui: {
    clipboard_backend: string
    microphone_device: string | null
    chunk_duration_secs: number
    output_method: OutputMethod
    autotype_backend: AutotypeBackend
    autotype_delay_ms: number | null
    vad: {
      enabled: boolean
      threshold: number
    }
    active_preset: string | null
    bubble: {
      enabled: boolean
    }
    model_memory: {
      keep_model_loaded: boolean
      unload_after_minutes: number
    }
  }
}

// How transcribed text should be output
export type OutputMethod = 'clipboard' | 'autotype' | 'both'

// Which backend to use for autotyping (when OutputMethod includes autotype)
export type AutotypeBackend = 'auto' | 'tools' | 'enigo'

// Status of autotyping tool availability
export interface AutotypeToolStatus {
  available: string[]
  recommended: string | null
  install_hint: string | null
}

// Shortcut backend information
export interface BackendInfo {
  backend: string
  requires_restart: boolean
  compositor: string
  portal_version: number
  is_flatpak: boolean
}

// Shortcut path mismatch information
export interface ShortcutPathMismatch {
  configured_command: string
  current_command: string
}

// Status response from backend
export interface StatusResponse {
  state: 'Idle' | 'Recording' | 'Transcribing'
  config_valid: boolean
}

// Response when saving settings
export interface SaveSettingsResponse {
  needs_restart: boolean
}

// Whisper model info from backend
export interface WhisperModelInfo {
  name: string
  description: string
  installed: boolean
  path: string
}

// Parakeet model info from backend
export interface ParakeetModelInfo {
  name: string
  description: string
  size: string
  installed: boolean
  path: string
}

// Preset info from backend
export interface PresetInfo {
  name: string
  description: string
  is_builtin: boolean
}

// Full preset details
export interface PresetDetails {
  name: string
  description: string
  prompt: string
  post_processor: string | null
  model: string | null
  is_builtin: boolean
}

// Cloud provider configuration
export interface CloudProviderInfo {
  value: Provider
  label: string
  keyUrl: string
  placeholder: string
}

// Helper to check if provider is local
export function isLocalProvider(provider: Provider): boolean {
  return provider === 'local-whisper' || provider === 'local-parakeet'
}

// Normalize provider to base variant (realtime variants → base)
export function normalizeProvider(provider: Provider): Provider {
  switch (provider) {
    case 'openai-realtime':
      return 'openai'
    case 'deepgram-realtime':
      return 'deepgram'
    default:
      return provider
  }
}
