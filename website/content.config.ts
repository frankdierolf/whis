import { defineCollection, defineContentConfig, z } from '@nuxt/content'

// Schema for blog posts
const blogSchema = z.object({
  title: z.string(),
  description: z.string(),
  date: z.string(),
  tags: z.array(z.string()).optional(),
  image: z.object({
    src: z.string(),
    alt: z.string(),
  }).optional(),
  author: z.string().optional(),
  draft: z.boolean().default(false),
})

// Locales supported by the site
const locales = ['en', 'zh', 'es', 'fr', 'de', 'pt', 'ru', 'ja', 'ko', 'it'] as const

// Generate collections for each locale
const blogCollections = Object.fromEntries(
  locales.map(locale => [
    `blog_${locale}`,
    defineCollection({
      type: 'page',
      source: `blog/${locale}/**/*.md`,
      schema: blogSchema,
    }),
  ]),
)

export default defineContentConfig({
  collections: blogCollections,
})
