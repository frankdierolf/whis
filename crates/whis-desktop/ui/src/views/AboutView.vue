<script setup lang="ts" vapor>
defineProps<{
  appVersion: string;
  appSite: string;
  appRepo: string;
}>();
</script>

<template>
  <section class="section">
    <header class="section-header">
      <h1>About</h1>
      <p>Credits & info</p>
    </header>

    <div class="section-content">
      <div class="about-brand">
        <span class="about-wordmark">whis</span>
        <span class="about-version">v{{ appVersion }}</span>
      </div>

      <p class="about-desc">
        Your voice, piped to clipboard.
      </p>

      <ul class="about-features">
        <li><span class="marker">[*]</span> Press shortcut, speak, release</li>
        <li><span class="marker">[*]</span> Whisper-powered transcription</li>
        <li><span class="marker">[*]</span> Auto-copied to clipboard</li>
      </ul>

      <div class="about-links">
        <a :href="appSite" target="_blank" class="about-link">whis.ink</a>
        <a :href="appRepo" target="_blank" class="about-link">GitHub</a>
      </div>
    </div>
  </section>
</template>

<style scoped>
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
  transition: color 0.15s ease;
}

.about-link:hover {
  color: var(--accent);
}
</style>
