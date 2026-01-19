<script setup lang="ts">
import type { LocaleCode } from '~/types/i18n'
import { computed, ref } from 'vue'

const { t, locale, locales } = useI18n()
const localePath = useLocalePath()
const switchLocalePath = useSwitchLocalePath()

const route = useRoute()
const sidebarOpen = ref(false)

// Add hreflang tags for SEO
const hreflangLinks = computed(() => {
  const links = (locales.value as Array<{ code: string, iso: string }>).map((loc) => {
    const path = switchLocalePath(loc.code as LocaleCode)
    return {
      rel: 'alternate',
      hreflang: loc.iso,
      href: `https://whis.ink${path}`,
    }
  })

  // Add x-default pointing to English (default locale, no prefix)
  const enPath = switchLocalePath('en')
  links.push({
    rel: 'alternate',
    hreflang: 'x-default',
    href: `https://whis.ink${enPath}`,
  })

  return links
})

// Set dynamic HTML lang attribute
useHead({
  htmlAttrs: {
    lang: () => locale.value,
  },
  link: hreflangLinks,
})

const navItems = computed(() => [
  { path: localePath('index'), name: 'index', label: t('nav.home') },
  { path: localePath('downloads'), name: 'downloads', label: t('nav.downloads') },
  { path: localePath('cli'), name: 'cli', label: t('nav.cli') },
  { path: localePath('desktop'), name: 'desktop', label: t('nav.desktop') },
  { path: localePath('mobile'), name: 'mobile', label: t('nav.mobile') },
  { path: localePath('faq'), name: 'faq', label: t('nav.faq') },
  { path: localePath('blog'), name: 'blog', label: t('nav.blog') },
])

function toggleSidebar() {
  sidebarOpen.value = !sidebarOpen.value
}

function closeSidebar() {
  sidebarOpen.value = false
}
</script>

<template>
  <div class="app" :class="{ 'sidebar-open': sidebarOpen }">
    <!-- Mobile Header -->
    <header class="mobile-header">
      <button
        class="menu-toggle"
        :aria-expanded="sidebarOpen"
        aria-label="Toggle menu"
        @click="toggleSidebar"
      >
        <span>{{ sidebarOpen ? '[x]' : '[=]' }}</span>
      </button>
      <span class="mobile-brand">whis</span>
    </header>

    <!-- Backdrop -->
    <div class="backdrop" @click="closeSidebar" />

    <!-- Sidebar -->
    <aside class="sidebar">
      <div class="brand">
        <span class="wordmark">whis</span>
      </div>

      <nav class="nav">
        <NuxtLink
          v-for="item in navItems"
          :key="item.name"
          :to="item.path"
          class="nav-item"
          :class="{ active: route.path === item.path }"
          @click="closeSidebar"
        >
          <span class="nav-marker" aria-hidden="true">{{
            route.path === item.path ? '>' : ' '
          }}</span>
          <span>{{ item.label }}</span>
        </NuxtLink>
      </nav>

      <div class="sidebar-footer">
        <LanguageSwitcher />
      </div>
    </aside>

    <!-- Content -->
    <main class="content">
      <slot />
    </main>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  min-height: 100vh;
  background: var(--bg);
}

/* Mobile Header - hidden on desktop */
.mobile-header {
  display: none;
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 48px;
  background: var(--bg);
  border-bottom: 1px solid var(--border);
  align-items: center;
  padding: 0 16px;
  gap: 12px;
  z-index: 101;
}

.menu-toggle {
  all: unset;
  font-family: var(--font);
  font-size: 14px;
  color: var(--text);
  cursor: pointer;
  padding: 4px 2px;
  transition: color 0.15s ease;
}

.menu-toggle:hover {
  color: var(--accent);
}

.mobile-brand {
  font-family: var(--font);
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-strong);
  letter-spacing: -0.03em;
}

/* Backdrop - hidden by default */
.backdrop {
  display: none;
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 99;
  opacity: 0;
  transition: opacity 0.2s ease;
  pointer-events: none;
}

/* Sidebar */
.sidebar {
  width: 140px;
  flex-shrink: 0;
  background: var(--bg);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  z-index: 100;
}

.brand {
  padding: 24px 16px 32px;
}

.wordmark {
  font-family: var(--font);
  font-size: 1.5rem;
  font-weight: 700;
  color: var(--text-strong);
  letter-spacing: -0.03em;
}

.nav {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-left: 2px solid transparent;
  color: var(--text-weak);
  font-family: var(--font);
  font-size: 13px;
  cursor: pointer;
  text-decoration: none;
  transition: all 0.15s ease;
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
  width: 0.75em;
}

.nav-item.active .nav-marker {
  color: var(--accent);
}

.sidebar-footer {
  padding: 16px;
  border-top: 1px solid var(--border);
  margin-top: auto;
}

/* Content */
.content {
  flex: 1;
  margin-left: 140px;
  min-height: 100vh;
  overflow-y: auto;
}

/* Mobile responsive (<768px) */
@media (max-width: 767px) {
  .mobile-header {
    display: flex;
  }

  .sidebar {
    width: 220px;
    top: 48px; /* Below mobile header */
    transform: translateX(-100%);
    transition: transform 0.25s ease;
  }

  .sidebar-open .sidebar {
    transform: translateX(0);
  }

  .brand {
    display: none; /* Hidden on mobile - brand is in header */
  }

  .backdrop {
    display: block;
    top: 48px; /* Below mobile header */
  }

  .sidebar-open .backdrop {
    opacity: 1;
    pointer-events: auto;
  }

  .content {
    margin-left: 0;
    margin-top: 48px;
  }

  /* Larger touch targets on mobile */
  .nav-item {
    padding: 14px 20px;
    font-size: 14px;
  }

  .sidebar-footer {
    padding: 16px 20px;
  }

  .version-badge {
    font-size: 12px;
  }
}
</style>
