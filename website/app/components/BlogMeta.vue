<script setup lang="ts">
defineProps<{
  date: string
  author?: string
  tags?: string[]
}>()

function formatDate(dateString: string) {
  const date = new Date(dateString)
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  })
}
</script>

<template>
  <div class="blog-meta">
    <time class="meta-date">{{ formatDate(date) }}</time>
    <span v-if="author" class="meta-author">
      <span class="separator">&middot;</span>
      {{ author }}
    </span>
    <div v-if="tags && tags.length" class="meta-tags">
      <span class="separator">&middot;</span>
      <span v-for="tag in tags" :key="tag" class="tag">{{ tag }}</span>
    </div>
  </div>
</template>

<style scoped>
.blog-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.85rem;
  color: var(--text-weak);
  margin-bottom: 1.5rem;
}

.separator {
  color: var(--text-weak);
}

.meta-tags {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.tag {
  font-size: 0.75rem;
  color: var(--accent);
  background: transparent;
  padding: 0.125rem 0.375rem;
  border: 1px solid var(--accent);
  border-radius: 2px;
}
</style>
