import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/**
 * Configuration for a specific bubble state.
 */
export interface StateConfig {
  /**
   * Icon resource name for this state (optional).
   * If not provided, uses the default icon from BubbleOptions.
   * Android drawable resource name (without "R.drawable." prefix).
   * Example: "ic_recording"
   */
  iconResourceName?: string
}

/**
 * Options for configuring the floating bubble.
 */
export interface BubbleOptions {
  /**
   * Size of the bubble in dp. Default: 60
   */
  size?: number

  /**
   * Initial X position. Default: 0
   */
  startX?: number

  /**
   * Initial Y position. Default: 100
   */
  startY?: number

  /**
   * Default icon resource name (used when no state-specific icon is provided).
   * Android drawable resource name (without "R.drawable." prefix).
   * If not specified, uses the plugin's default icon.
   * Example: "ic_my_app_logo"
   */
  iconResourceName?: string

  /**
   * Background color (hex string). Default: "#1C1C1C" (dark)
   */
  background?: string

  /**
   * State configuration mapping.
   * Keys are arbitrary state names, values define icon for that state.
   *
   * @example
   * ```typescript
   * states: {
   *   'idle': { iconResourceName: 'ic_idle' },
   *   'recording': { iconResourceName: 'ic_recording' },
   *   'processing': { iconResourceName: 'ic_processing' }
   * }
   * ```
   */
  states?: Record<string, StateConfig>
}

/**
 * Response from visibility check.
 */
export interface VisibilityResponse {
  visible: boolean
}

/**
 * Response from permission check.
 */
export interface PermissionResponse {
  granted: boolean
}

/**
 * Show the floating bubble overlay.
 *
 * @param options - Configuration options for the bubble
 * @throws If overlay permission is not granted
 *
 * @example
 * ```typescript
 * import { showBubble } from 'tauri-plugin-floating-bubble'
 *
 * // Basic usage with defaults
 * await showBubble()
 *
 * // With custom icon and states
 * await showBubble({
 *   size: 60,
 *   startX: 0,
 *   startY: 200,
 *   iconResourceName: 'ic_my_logo',
 *   background: '#1C1C1C',
 *   states: {
 *     'idle': { iconResourceName: 'ic_idle' },
 *     'recording': { iconResourceName: 'ic_recording' },
 *     'processing': { iconResourceName: 'ic_processing' },
 *   }
 * })
 * ```
 */
export async function showBubble(options?: BubbleOptions): Promise<void> {
  await invoke('plugin:floating-bubble|show_bubble', { options })
}

/**
 * Hide the floating bubble overlay.
 *
 * @example
 * ```typescript
 * import { hideBubble } from 'tauri-plugin-floating-bubble'
 * await hideBubble()
 * ```
 */
export async function hideBubble(): Promise<void> {
  await invoke('plugin:floating-bubble|hide_bubble')
}

/**
 * Check if the floating bubble is currently visible.
 *
 * @returns Whether the bubble is visible
 *
 * @example
 * ```typescript
 * import { isBubbleVisible } from 'tauri-plugin-floating-bubble'
 * const { visible } = await isBubbleVisible()
 * ```
 */
export async function isBubbleVisible(): Promise<VisibilityResponse> {
  return await invoke<VisibilityResponse>('plugin:floating-bubble|is_bubble_visible')
}

/**
 * Request the overlay permission (SYSTEM_ALERT_WINDOW).
 * Opens system settings if permission is not granted.
 *
 * @returns Whether permission was granted
 *
 * @example
 * ```typescript
 * import { requestOverlayPermission } from 'tauri-plugin-floating-bubble'
 * const { granted } = await requestOverlayPermission()
 * if (granted) {
 *   await showBubble()
 * }
 * ```
 */
export async function requestOverlayPermission(): Promise<PermissionResponse> {
  return await invoke<PermissionResponse>('plugin:floating-bubble|request_overlay_permission')
}

/**
 * Check if the overlay permission (SYSTEM_ALERT_WINDOW) is granted.
 *
 * @returns Whether permission is granted
 *
 * @example
 * ```typescript
 * import { hasOverlayPermission } from 'tauri-plugin-floating-bubble'
 * const { granted } = await hasOverlayPermission()
 * ```
 */
export async function hasOverlayPermission(): Promise<PermissionResponse> {
  return await invoke<PermissionResponse>('plugin:floating-bubble|has_overlay_permission')
}

/**
 * Request the microphone permission (RECORD_AUDIO).
 * Opens the system permission dialog.
 *
 * This permission is required on Android 14+ for foreground services with microphone type.
 *
 * @returns Whether permission was granted (returns false if dialog was shown)
 *
 * @example
 * ```typescript
 * import { requestMicrophonePermission, hasMicrophonePermission } from 'tauri-plugin-floating-bubble'
 * await requestMicrophonePermission()
 * // After user responds, check if granted
 * const { granted } = await hasMicrophonePermission()
 * ```
 */
export async function requestMicrophonePermission(): Promise<PermissionResponse> {
  return await invoke<PermissionResponse>('plugin:floating-bubble|request_microphone_permission')
}

/**
 * Check if the microphone permission (RECORD_AUDIO) is granted.
 *
 * This permission is required on Android 14+ for foreground services with microphone type.
 *
 * @returns Whether permission is granted
 *
 * @example
 * ```typescript
 * import { hasMicrophonePermission } from 'tauri-plugin-floating-bubble'
 * const { granted } = await hasMicrophonePermission()
 * ```
 */
export async function hasMicrophonePermission(): Promise<PermissionResponse> {
  return await invoke<PermissionResponse>('plugin:floating-bubble|has_microphone_permission')
}

/**
 * Update the bubble's visual state.
 *
 * @param state - The state name to set. Must be a key in the states map provided to showBubble.
 *
 * @example
 * ```typescript
 * import { setBubbleState } from 'tauri-plugin-floating-bubble'
 * await setBubbleState('idle')
 * await setBubbleState('recording')
 * await setBubbleState('processing')
 * ```
 */
export async function setBubbleState(state: string): Promise<void> {
  await invoke('plugin:floating-bubble|set_bubble_state', { state })
}

/**
 * Event payload when the bubble is clicked.
 */
export interface BubbleClickEvent {
  /** The action that triggered the event */
  action: 'click'
}

/**
 * The event name used for bubble click events.
 * Can be used with `listen()` from `@tauri-apps/api/event` directly.
 */
export const BUBBLE_CLICK_EVENT = 'floating-bubble://click'

/**
 * Register a listener for bubble click events.
 *
 * This uses Tauri's global event system (same pattern as official plugins like plugin-store).
 *
 * @param callback - Function to call when the bubble is clicked
 * @returns A function to unregister the listener
 *
 * @example
 * ```typescript
 * import { onBubbleClick } from 'tauri-plugin-floating-bubble'
 *
 * const unlisten = await onBubbleClick((event) => {
 *   console.log('Bubble clicked!', event.action)
 *   // Start/stop recording, etc.
 * })
 *
 * // Later, to stop listening:
 * unlisten()
 * ```
 */
export async function onBubbleClick(
  callback: (event: BubbleClickEvent) => void,
): Promise<() => void> {
  return listen(BUBBLE_CLICK_EVENT, (event) => {
    callback(event.payload as BubbleClickEvent)
  })
}

/**
 * Event payload when the bubble is closed via drag-to-close.
 */
export interface BubbleCloseEvent {
  /** The action that triggered the event */
  action: 'close'
}

/**
 * The event name used for bubble close events.
 * Can be used with `listen()` from `@tauri-apps/api/event` directly.
 */
export const BUBBLE_CLOSE_EVENT = 'floating-bubble://close'

/**
 * Register a listener for bubble close events.
 *
 * This is triggered when the user drags the bubble to the close zone
 * at the bottom center of the screen.
 *
 * @param callback - Function to call when the bubble is closed
 * @returns A function to unregister the listener
 *
 * @example
 * ```typescript
 * import { onBubbleClose, hideBubble } from 'tauri-plugin-floating-bubble'
 *
 * const unlisten = await onBubbleClose(async (event) => {
 *   console.log('Bubble closed via drag-to-close!', event.action)
 *   // Update settings to disable bubble, etc.
 * })
 *
 * // Later, to stop listening:
 * unlisten()
 * ```
 */
export async function onBubbleClose(
  callback: (event: BubbleCloseEvent) => void,
): Promise<() => void> {
  return listen(BUBBLE_CLOSE_EVENT, (event) => {
    callback(event.payload as BubbleCloseEvent)
  })
}
