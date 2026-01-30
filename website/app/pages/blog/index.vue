<script setup lang="ts">
import { computed } from 'vue'

const { t, locale } = useI18n()
const route = useRoute()

const canonicalUrl = computed(() => `https://whis.ink${route.path}`)

useHead({
  title: t('blog.title'),
  link: [
    { rel: 'canonical', href: canonicalUrl },
  ],
  meta: [
    { name: 'description', content: t('blog.metaDescription') },
    { property: 'og:title', content: t('blog.title') },
    { property: 'og:description', content: t('blog.metaDescription') },
    { property: 'og:url', content: canonicalUrl },
    { property: 'og:image', content: 'https://whis.ink/og-image.jpg' },
    { property: 'og:type', content: 'website' },
    { name: 'twitter:card', content: 'summary_large_image' },
    { name: 'twitter:title', content: t('blog.title') },
    { name: 'twitter:description', content: t('blog.metaDescription') },
    { name: 'twitter:image', content: 'https://whis.ink/og-image.jpg' },
  ],
})

// Query blog posts for current locale with fallback to English
const currentCollection = `blog_${locale.value}` as const
const fallbackCollection = 'blog_en' as const

// Try current locale first
const { data: localePosts } = await useAsyncData(
  `blog-${locale.value}`,
  () => queryCollection(currentCollection)
    .where('draft', '!=', true)
    .order('date', 'DESC')
    .all()
    .catch(() => []),
)

// If no locale posts, use English fallback
const { data: fallbackPosts } = await useAsyncData(
  'blog-en-fallback',
  () => locale.value !== 'en'
    ? queryCollection(fallbackCollection)
      .where('draft', '!=', true)
      .order('date', 'DESC')
      .all()
      .catch(() => [])
    : Promise.resolve([]),
)

// Use locale posts if available, otherwise fallback to English
const posts = computed(() => {
  const localList = localePosts.value || []
  const fallbackList = fallbackPosts.value || []
  return localList.length > 0 ? localList : fallbackList
})

// Extract slug from path (e.g., /blog/en/hello-world -> hello-world)
function getSlug(path: string) {
  const segments = path.split('/')
  return segments[segments.length - 1]
}
</script>

<template>
  <div class="blog-content">
    <ViewHeader :title="$t('blog.title').replace(' - whis', '')" :subtitle="$t('blog.subtitle')" />

    <section class="blog-list">
      <template v-if="posts.length">
        <div class="blog-grid">
          <BlogCard
            v-for="post in posts"
            :key="post.path"
            :title="post.title"
            :description="post.description"
            :date="post.date"
            :slug="getSlug(post.path)"
            :tags="post.tags"
            :image="post.image"
          />
        </div>
      </template>
      <p v-else class="no-posts">
        {{ $t('blog.noPosts') }}
      </p>
    </section>
  </div>
</template>

<style scoped>
.blog-content {
  padding: 2rem;
}

.blog-list {
  padding: var(--vertical-padding) 0;
}

.blog-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 1.5rem;
}

.no-posts {
  color: var(--text-weak);
  font-size: 0.9rem;
}
</style>
