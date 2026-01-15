<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'

const appVersion = '0.7.0'
const appSite = 'https://whis.ink'
const appRepo = 'https://github.com/frankdierolf/whis'
</script>

<template>
  <section class="section">
    <div class="section-content">
      <div class="about-brand">
        <span class="about-wordmark">whis</span>
        <span class="about-version">v{{ appVersion }}</span>
      </div>

      <p class="about-desc">
        Your voice, piped to clipboard.
      </p>

      <ul class="about-features">
        <li><span class="marker">[*]</span> Tap to record, tap to stop</li>
        <li><span class="marker">[*]</span> Multi-provider transcription</li>
        <li><span class="marker">[*]</span> Auto-copied to clipboard</li>
      </ul>

      <div class="about-links">
        <span class="about-link" @click="openUrl(appSite)">whis.ink</span>
        <span class="about-link" @click="openUrl(appRepo)">GitHub</span>
      </div>

      <p class="about-author">
        Made by Frank Dierolf
      </p>
    </div>
  </section>
</template>

<style scoped>
.section {
  padding: 20px;
}

.section-content {
  display: flex;
  flex-direction: column;
}

.about-brand {
  display: flex;
  align-items: baseline;
  gap: 12px;
  margin-bottom: 12px;
}

.about-wordmark {
  font-family: var(--font);
  font-size: 1.75rem;
  font-weight: 700;
  color: var(--text-strong);
  letter-spacing: -0.02em;
}

.about-version {
  font-size: 12px;
  color: var(--text-weak);
}

.about-desc {
  font-size: 13px;
  color: var(--text);
  margin-bottom: 16px;
}

.about-features {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 20px;
}

.about-features li {
  display: flex;
  gap: 8px;
  font-size: 12px;
  color: var(--text);
}

.marker {
  color: var(--icon);
}

.about-links {
  display: flex;
  gap: 16px;
  margin-bottom: 24px;
}

.about-link {
  font-size: 12px;
  color: var(--text-strong);
  text-decoration: underline;
  text-underline-offset: 2px;
  text-decoration-thickness: 1px;
  cursor: pointer;
  transition: color 0.15s ease;
}

.about-link:active {
  color: var(--accent);
}

.about-author {
  font-size: 12px;
  color: var(--text-weak);
}
</style>
