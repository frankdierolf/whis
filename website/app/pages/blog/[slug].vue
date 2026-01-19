<script setup lang="ts">
import { computed } from 'vue'

const { locale } = useI18n()
const route = useRoute()
const localePath = useLocalePath()

const slug = computed(() => route.params.slug as string)
const canonicalUrl = computed(() => `https://whis.ink${route.path}`)

// Query blog post for current locale with fallback to English
const currentCollection = `blog_${locale.value}` as const
const fallbackCollection = 'blog_en' as const

// Try current locale first
const { data: localePost } = await useAsyncData(
  `blog-post-${locale.value}-${slug.value}`,
  () => queryCollection(currentCollection)
    .where('path', 'LIKE', `%/${slug.value}`)
    .first()
    .catch(() => null),
)

// Fallback to English if no locale post found
const { data: fallbackPost } = await useAsyncData(
  `blog-post-en-fallback-${slug.value}`,
  () => !localePost.value && locale.value !== 'en'
    ? queryCollection(fallbackCollection)
      .where('path', 'LIKE', `%/${slug.value}`)
      .first()
      .catch(() => null)
    : Promise.resolve(null),
)

const post = computed(() => localePost.value || fallbackPost.value)

// 404 if post not found
if (!post.value) {
  throw createError({
    statusCode: 404,
    statusMessage: 'Post not found',
  })
}

// SEO meta
useHead({
  title: `${post.value.title} - whis`,
  link: [
    { rel: 'canonical', href: canonicalUrl },
  ],
  meta: [
    { name: 'description', content: post.value.description },
    { property: 'og:title', content: post.value.title },
    { property: 'og:description', content: post.value.description },
    { property: 'og:url', content: canonicalUrl },
    { property: 'og:image', content: post.value.image?.src || 'https://whis.ink/og-image.jpg' },
    { property: 'og:type', content: 'article' },
    { property: 'article:published_time', content: post.value.date },
    { name: 'twitter:card', content: 'summary_large_image' },
    { name: 'twitter:title', content: post.value.title },
    { name: 'twitter:description', content: post.value.description },
    { name: 'twitter:image', content: post.value.image?.src || 'https://whis.ink/og-image.jpg' },
  ],
})
</script>

<template>
  <div class="blog-post-content">
    <NuxtLink :to="localePath('/blog')" class="back-link">
      &larr; Back to blog
    </NuxtLink>

    <article v-if="post" class="blog-post">
      <header class="post-header">
        <h1 class="post-title">{{ post.title }}</h1>
        <BlogMeta
          :date="post.date"
          :author="post.author"
          :tags="post.tags"
        />
      </header>

      <div v-if="post.image" class="post-image">
        <img :src="post.image.src" :alt="post.image.alt">
      </div>

      <div class="prose">
        <ContentRenderer :value="post" />
      </div>
    </article>
  </div>
</template>

<style scoped>
.blog-post-content {
  padding: 2rem;
  max-width: var(--max-width);
}

.back-link {
  display: inline-block;
  font-size: 0.85rem;
  color: var(--text-weak);
  text-decoration: none;
  margin-bottom: 2rem;
  transition: color 0.15s ease;
}

.back-link:hover {
  color: var(--accent);
}

.post-header {
  margin-bottom: 2rem;
  padding-bottom: 1.5rem;
  border-bottom: 1px solid var(--border-weak);
}

.post-title {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-strong);
  margin-bottom: 1rem;
  line-height: 1.3;
}

.post-image {
  margin-bottom: 2rem;
  border-radius: 4px;
  overflow: hidden;
}

.post-image img {
  width: 100%;
  height: auto;
}

/* Prose styles for markdown content */
.prose {
  color: var(--text);
  line-height: 1.7;
}

.prose :deep(h1),
.prose :deep(h2),
.prose :deep(h3),
.prose :deep(h4),
.prose :deep(h5),
.prose :deep(h6) {
  color: var(--text-strong);
  font-weight: 600;
  margin-top: 2rem;
  margin-bottom: 1rem;
}

.prose :deep(h1) {
  font-size: 1.5rem;
}

.prose :deep(h2) {
  font-size: 1.25rem;
}

.prose :deep(h3) {
  font-size: 1.1rem;
}

.prose :deep(p) {
  margin-bottom: 1rem;
}

.prose :deep(a) {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 0.2em;
}

.prose :deep(a:hover) {
  color: var(--text-strong);
}

.prose :deep(ul),
.prose :deep(ol) {
  margin-bottom: 1rem;
  padding-left: 1.5rem;
}

.prose :deep(ul) {
  list-style: disc;
}

.prose :deep(ol) {
  list-style: decimal;
}

.prose :deep(li) {
  margin-bottom: 0.5rem;
}

.prose :deep(code) {
  font-family: var(--font);
  background: var(--bg-weak);
  padding: 0.125rem 0.375rem;
  border-radius: 2px;
  font-size: 0.9em;
}

.prose :deep(pre) {
  background: var(--bg-weak);
  padding: 1rem;
  border-radius: 4px;
  overflow-x: auto;
  margin-bottom: 1rem;
}

.prose :deep(pre code) {
  background: transparent;
  padding: 0;
}

.prose :deep(blockquote) {
  border-left: 3px solid var(--accent);
  padding-left: 1rem;
  margin: 1rem 0;
  color: var(--text-weak);
  font-style: italic;
}

.prose :deep(strong) {
  color: var(--text-strong);
  font-weight: 600;
}

.prose :deep(hr) {
  border: none;
  border-top: 1px solid var(--border-weak);
  margin: 2rem 0;
}

.prose :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 4px;
  margin: 1rem 0;
}

@media (max-width: 767px) {
  .post-title {
    font-size: 1.25rem;
  }
}
</style>
