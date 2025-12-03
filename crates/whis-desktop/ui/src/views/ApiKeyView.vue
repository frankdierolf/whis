<script setup lang="ts" vapor>
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface SaveResult {
  needs_restart: boolean;
}

const props = defineProps<{
  currentShortcut: string;
  modelValue: string;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

const apiKeyMasked = ref(true);
const status = ref("");

async function saveApiKey() {
  try {
    // Validate format if key is provided
    if (props.modelValue && !props.modelValue.startsWith('sk-')) {
      status.value = "Invalid key format. Keys start with 'sk-'";
      return;
    }

    await invoke<SaveResult>('save_settings', {
      settings: {
        shortcut: props.currentShortcut,
        openai_api_key: props.modelValue || null
      }
    });
    status.value = "Saved";
    setTimeout(() => status.value = "", 2000);
  } catch (e) {
    status.value = "Failed to save: " + e;
  }
}
</script>

<template>
  <section class="section">
    <header class="section-header">
      <h1>API Keys</h1>
      <p>Configure your OpenAI API key</p>
    </header>

    <div class="section-content">
      <div class="field">
        <label>OpenAI API Key</label>
        <div class="api-key-input">
          <input
            :type="apiKeyMasked ? 'password' : 'text'"
            :value="modelValue"
            @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
            placeholder="sk-..."
            spellcheck="false"
            autocomplete="off"
          />
          <button @click="apiKeyMasked = !apiKeyMasked" class="toggle-btn" type="button">
            {{ apiKeyMasked ? 'show' : 'hide' }}
          </button>
        </div>
        <p class="hint">
          Get your key from
          <a href="https://platform.openai.com/api-keys" target="_blank">platform.openai.com</a>
        </p>
      </div>

      <button @click="saveApiKey" class="btn btn-secondary">Save</button>

      <div class="status" :class="{ visible: status }">{{ status }}</div>

      <div class="notice">
        <span class="notice-marker">[i]</span>
        <p>Stored locally in ~/.config/whis/settings.json</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.api-key-input {
  display: flex;
  gap: 8px;
}

.api-key-input input {
  flex: 1;
  padding: 10px 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-family: var(--font);
  font-size: 12px;
  color: var(--text);
  transition: border-color 0.15s ease;
}

.api-key-input input::placeholder {
  color: var(--text-weak);
}

.api-key-input input:focus {
  outline: none;
  border-color: var(--accent);
}

.toggle-btn {
  padding: 10px 12px;
  background: var(--bg-weak);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-family: var(--font);
  font-size: 11px;
  color: var(--text-weak);
  cursor: pointer;
  transition: all 0.15s ease;
}

.toggle-btn:hover {
  border-color: var(--text-weak);
  color: var(--text);
}
</style>
