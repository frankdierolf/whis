<script setup lang="ts">
defineProps<{
  title: string
  description: string
  date: string
  slug: string
  tags?: string[]
  image?: {
    src: string
    alt: string
  }
}>()

const localePath = useLocalePath()

function formatDate(dateString: string) {
  const date = new Date(dateString)
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}
</script>

<template>
  <article class="blog-card">
    <NuxtLink :to="localePath(`/blog/${slug}`)" class="card-link">
      <div v-if="image" class="card-image">
        <img :src="image.src" :alt="image.alt">
      </div>
      <div class="card-content">
        <time class="card-date">{{ formatDate(date) }}</time>
        <h2 class="card-title">{{ title }}</h2>
        <p class="card-description">{{ description }}</p>
        <div v-if="tags && tags.length" class="card-tags">
          <span v-for="tag in tags" :key="tag" class="tag">{{ tag }}</span>
        </div>
      </div>
    </NuxtLink>
  </article>
</template>

<style scoped>
.blog-card {
  background: var(--bg-weak);
  border: 1px solid var(--border-weak);
  border-radius: 4px;
  overflow: hidden;
  transition: border-color 0.15s ease;
}

.blog-card:hover {
  border-color: var(--accent);
}

.card-link {
  display: block;
  text-decoration: none;
  color: inherit;
}

.card-image {
  aspect-ratio: 16 / 9;
  overflow: hidden;
}

.card-image img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.card-content {
  padding: 1.25rem;
}

.card-date {
  display: block;
  font-size: 0.75rem;
  color: var(--text-weak);
  margin-bottom: 0.5rem;
}

.card-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-strong);
  margin-bottom: 0.5rem;
  line-height: 1.4;
}

.card-description {
  font-size: 0.85rem;
  color: var(--text);
  line-height: 1.5;
  margin-bottom: 0.75rem;
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.tag {
  font-size: 0.7rem;
  color: var(--accent);
  background: transparent;
  padding: 0.125rem 0.375rem;
  border: 1px solid var(--accent);
  border-radius: 2px;
}
</style>
