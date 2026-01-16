<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, ref } from 'vue'

type BubbleState = 'idle' | 'recording' | 'transcribing'

const state = ref<BubbleState>('idle')
const isVisible = ref(false)

// Drag state
const isDragging = ref(false)
const dragStart = ref({ x: 0, y: 0 })
const hasMoved = ref(false)
const DRAG_THRESHOLD = 5 // pixels before considering it a drag

// Map state to SVG icon
const iconSrc = computed(() => {
  switch (state.value) {
    case 'recording':
      return '/icons/bubble-recording.svg'
    case 'transcribing':
      return '/icons/bubble-processing.svg'
    default:
      return '/icons/bubble-idle.svg'
  }
})

let unlistenState: (() => void) | null = null
let unlistenHide: (() => void) | null = null

onMounted(async () => {
  // Listen for state changes from Rust
  unlistenState = await listen<BubbleState>('bubble-state', (event) => {
    state.value = event.payload
    isVisible.value = true
  })

  // Listen for hide signal
  unlistenHide = await listen('bubble-hide', () => {
    isVisible.value = false
  })
})

onUnmounted(() => {
  unlistenState?.()
  unlistenHide?.()
})

function handleMouseDown(e: MouseEvent) {
  isDragging.value = true
  hasMoved.value = false
  dragStart.value = { x: e.screenX, y: e.screenY }

  // Use window-level events so drag works even when cursor leaves the bubble
  window.addEventListener('mousemove', handleWindowMouseMove)
  window.addEventListener('mouseup', handleWindowMouseUp)
}

function handleWindowMouseMove(e: MouseEvent) {
  if (!isDragging.value)
    return

  const deltaX = e.screenX - dragStart.value.x
  const deltaY = e.screenY - dragStart.value.y

  // Only start moving after exceeding threshold
  if (!hasMoved.value && (Math.abs(deltaX) > DRAG_THRESHOLD || Math.abs(deltaY) > DRAG_THRESHOLD)) {
    hasMoved.value = true
  }

  if (hasMoved.value) {
    dragStart.value = { x: e.screenX, y: e.screenY }
    invoke('bubble_move_by', { dx: deltaX, dy: deltaY }).catch((err) => {
      console.error('Failed to move bubble:', err)
    })
  }
}

async function handleWindowMouseUp(_e: MouseEvent) {
  // Remove window-level listeners
  window.removeEventListener('mousemove', handleWindowMouseMove)
  window.removeEventListener('mouseup', handleWindowMouseUp)

  if (isDragging.value) {
    isDragging.value = false
    if (hasMoved.value) {
      // Save the new position after dragging
      try {
        const pos = await invoke<{ x: number, y: number }>('bubble_get_position')
        await invoke('bubble_save_position', { x: pos.x, y: pos.y })
      }
      catch (err) {
        console.error('Failed to save bubble position:', err)
      }
    }
    else {
      // No movement - treat as a click
      invoke('bubble_toggle_recording')
    }
  }
}
</script>

<template>
  <div
    class="bubble"
    :class="{
      visible: isVisible,
      recording: state === 'recording',
      transcribing: state === 'transcribing',
      dragging: isDragging && hasMoved,
    }"
    @mousedown="handleMouseDown"
  >
    <img :src="iconSrc" alt="Whis" class="icon" draggable="false">
  </div>
</template>

<style scoped>
.bubble {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0;
  transition: opacity 200ms ease, transform 100ms ease, box-shadow 200ms ease;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
}

.bubble.visible {
  opacity: 1;
}

/* Recording and transcribing states use same shadow as idle - icon color indicates state */

.bubble:hover {
  transform: scale(1.05);
}

.bubble:active {
  transform: scale(0.95);
}

.bubble.dragging {
  cursor: grabbing;
  transform: scale(1.05);
  opacity: 0.9;
}

.icon {
  width: 32px;
  height: 32px;
}
</style>
