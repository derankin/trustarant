import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import express from 'express'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const distDir = path.join(__dirname, 'dist')
const indexPath = path.join(distDir, 'index.html')

const app = express()
const port = Number(process.env.PORT || 8080)
const apiBaseUrl = (process.env.API_BASE_URL || process.env.VITE_API_BASE_URL || '').replace(/\/$/, '')
const publicBaseUrl = (process.env.PUBLIC_BASE_URL || 'https://cleanplated.com').replace(/\/$/, '')
const appTitle = 'CleanPlated'
const defaultDescription =
  'CleanPlated helps you find safer food with Southern California restaurant health inspection data.'
const defaultImage = `${publicBaseUrl}/social-card.png`

const indexHtml = fs.readFileSync(indexPath, 'utf8')

const escapeHtml = (value) =>
  String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const withMetaTags = (html, tags) => {
  const dynamicBlock = `\n<!-- cleanplated:ssr-meta:start -->\n${tags}\n<!-- cleanplated:ssr-meta:end -->\n`
  if (html.includes('<!-- cleanplated:ssr-meta:start -->')) {
    return html.replace(
      /<!-- cleanplated:ssr-meta:start -->[\s\S]*<!-- cleanplated:ssr-meta:end -->/,
      dynamicBlock.trim(),
    )
  }
  return html.replace('</head>', `${dynamicBlock}</head>`)
}

const buildMetaTags = ({ title, description, url, image }) => {
  const safeTitle = escapeHtml(title)
  const safeDescription = escapeHtml(description)
  const safeUrl = escapeHtml(url)
  const safeImage = escapeHtml(image)
  return [
    `<meta property="og:type" content="website" />`,
    `<meta property="og:site_name" content="${appTitle}" />`,
    `<meta property="og:title" content="${safeTitle}" />`,
    `<meta property="og:description" content="${safeDescription}" />`,
    `<meta property="og:url" content="${safeUrl}" />`,
    `<meta property="og:image" content="${safeImage}" />`,
    `<meta property="og:image:secure_url" content="${safeImage}" />`,
    `<meta property="og:image:type" content="image/png" />`,
    `<meta property="og:image:width" content="1200" />`,
    `<meta property="og:image:height" content="630" />`,
    `<meta property="og:image:alt" content="${safeTitle}" />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
    `<meta name="twitter:title" content="${safeTitle}" />`,
    `<meta name="twitter:description" content="${safeDescription}" />`,
    `<meta name="twitter:image" content="${safeImage}" />`,
    `<link rel="canonical" href="${safeUrl}" />`,
  ].join('\n')
}

const defaultMetaHtml = withMetaTags(
  indexHtml,
  buildMetaTags({
    title: `${appTitle} · Find safer food, faster.`,
    description: defaultDescription,
    url: `${publicBaseUrl}/`,
    image: defaultImage,
  }),
)

const fetchFacilityDetail = async (id) => {
  if (!apiBaseUrl) return null

  const response = await fetch(`${apiBaseUrl}/api/v1/facilities/${encodeURIComponent(id)}`, {
    headers: { Accept: 'application/json' },
  })
  if (!response.ok) return null

  const payload = await response.json()
  return payload?.data ?? null
}

app.get('/share/f/:facilityId', async (req, res) => {
  const { facilityId } = req.params
  const shareUrl = `${publicBaseUrl}/share/f/${encodeURIComponent(facilityId)}`

  try {
    const facility = await fetchFacilityDetail(facilityId)
    if (!facility) {
      const notFoundMeta = withMetaTags(
        indexHtml,
        buildMetaTags({
          title: `${appTitle} · Restaurant Health Data`,
          description: defaultDescription,
          url: shareUrl,
          image: defaultImage,
        }),
      )
      res.set('Cache-Control', 'public, max-age=120')
      return res.status(404).send(notFoundMeta)
    }

    const title = `${facility.name} · Trust Score ${facility.trust_score}`
    const description = `${facility.address}, ${facility.city} ${facility.postal_code} · ${facility.jurisdiction}`
    const html = withMetaTags(
      indexHtml,
      buildMetaTags({
        title,
        description,
        url: shareUrl,
        image: defaultImage,
      }),
    )

    res.set('Cache-Control', 'public, max-age=300')
    return res.send(html)
  } catch (error) {
    console.error('share metadata render failed', error)
    res.set('Cache-Control', 'no-cache')
    return res.send(defaultMetaHtml)
  }
})

app.use(express.static(distDir, { index: false, maxAge: '1h' }))

app.get(/.*/, (_req, res) => {
  res.set('Cache-Control', 'no-cache')
  res.send(defaultMetaHtml)
})

app.listen(port, () => {
  console.log(`cleanplated frontend server listening on :${port}`)
})
