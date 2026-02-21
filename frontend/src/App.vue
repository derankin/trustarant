<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

type FacilitySummary = {
  id: string
  name: string
  address: string
  city: string
  state: string
  postal_code: string
  latitude: number
  longitude: number
  jurisdiction: string
  trust_score: number
  latest_inspection_at?: string
}

type ConnectorIngestionStatus = {
  source: string
  fetched_records: number
  error?: string | null
}

type IngestionStats = {
  last_refresh_at?: string
  unique_facilities: number
  connector_stats: ConnectorIngestionStatus[]
}

type SortMode = 'trust_desc' | 'recent_desc' | 'name_asc'
type ScoreSlice = 'all' | 'elite' | 'solid' | 'watch'

const fallbackLatitude = 34.0522
const fallbackLongitude = -118.2437
const fallbackLabel = 'Downtown Los Angeles'

const facilities = ref<FacilitySummary[]>([])
const ingestionStats = ref<IngestionStats | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const search = ref('')
const radiusMiles = ref(2)
const jurisdictionFilter = ref('all')
const sortMode = ref<SortMode>('trust_desc')
const scoreSlice = ref<ScoreSlice>('all')
const recentOnly = ref(false)
const userLocation = ref<{ latitude: number; longitude: number; accuracy: number } | null>(null)
const locationState = ref<'fallback' | 'requesting' | 'granted' | 'denied' | 'unsupported'>(
  'fallback',
)
const locationMessage = ref('Using Los Angeles fallback center.')

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8080'

const activeCenter = computed(() => {
  if (userLocation.value) {
    return {
      latitude: userLocation.value.latitude,
      longitude: userLocation.value.longitude,
      label: 'Your browser location',
    }
  }

  return {
    latitude: fallbackLatitude,
    longitude: fallbackLongitude,
    label: fallbackLabel,
  }
})

const hasKeywordQuery = computed(() => search.value.trim().length > 0)

const scoreBandMeta = (score: number) => {
  if (score >= 90) return { label: 'Elite', className: 'cds--tag--green' }
  if (score >= 80) return { label: 'Solid', className: 'cds--tag--warm-gray' }
  return { label: 'Watch', className: 'cds--tag--red' }
}

const scoreSlices = computed(() => {
  const counts = { elite: 0, solid: 0, watch: 0 }

  for (const facility of facilities.value) {
    if (facility.trust_score >= 90) counts.elite += 1
    else if (facility.trust_score >= 80) counts.solid += 1
    else counts.watch += 1
  }

  return counts
})

const jurisdictionOptions = computed(() => {
  const jurisdictions = [...new Set(facilities.value.map((facility) => facility.jurisdiction))]
    .sort((left, right) => left.localeCompare(right))
    .map((jurisdiction) => ({
      label: jurisdiction,
      value: jurisdiction,
    }))

  return [{ label: 'All jurisdictions', value: 'all' }, ...jurisdictions]
})

const filteredFacilities = computed(() => {
  const now = Date.now()
  const ninetyDaysMs = 90 * 24 * 60 * 60 * 1000

  const filtered = facilities.value.filter((facility) => {
    if (jurisdictionFilter.value !== 'all' && facility.jurisdiction !== jurisdictionFilter.value) {
      return false
    }

    if (scoreSlice.value === 'elite' && facility.trust_score < 90) return false
    if (scoreSlice.value === 'solid' && (facility.trust_score < 80 || facility.trust_score >= 90)) {
      return false
    }
    if (scoreSlice.value === 'watch' && facility.trust_score >= 80) return false

    if (recentOnly.value) {
      if (!facility.latest_inspection_at) return false
      const inspectedAt = new Date(facility.latest_inspection_at).getTime()
      if (Number.isNaN(inspectedAt) || now - inspectedAt > ninetyDaysMs) return false
    }

    return true
  })

  filtered.sort((left, right) => {
    switch (sortMode.value) {
      case 'recent_desc': {
        const leftDate = left.latest_inspection_at ? new Date(left.latest_inspection_at).getTime() : 0
        const rightDate = right.latest_inspection_at ? new Date(right.latest_inspection_at).getTime() : 0
        return rightDate - leftDate
      }
      case 'name_asc':
        return left.name.localeCompare(right.name)
      default:
        return right.trust_score - left.trust_score
    }
  })

  return filtered
})

const featured = computed(() => filteredFacilities.value[0])

const lastRefreshLabel = computed(() => {
  if (!ingestionStats.value?.last_refresh_at) return 'Awaiting first successful ingestion'
  return new Date(ingestionStats.value.last_refresh_at).toLocaleString()
})

const connectorRows = computed(() => ingestionStats.value?.connector_stats ?? [])

const formatSourceName = (source: string) => {
  const labels: Record<string, string> = {
    la_county_open_data: 'Los Angeles County Open Data',
    san_diego_socrata: 'San Diego Socrata API',
    long_beach_closures_page: 'Long Beach Public Health',
    lives_batch_riv_sbc: 'Riverside + San Bernardino LIVES/ArcGIS',
    cpra_import_orange_pasadena: 'Orange County + Pasadena CPRA',
  }

  return labels[source] ?? source.replace(/_/g, ' ')
}

const formatDate = (value?: string) => {
  if (!value) return 'No date available'
  return new Date(value).toLocaleDateString()
}

const haversineMiles = (lat1: number, lon1: number, lat2: number, lon2: number) => {
  const radius = 3958.8
  const dLat = (lat2 - lat1) * (Math.PI / 180)
  const dLon = (lon2 - lon1) * (Math.PI / 180)
  const a =
    Math.sin(dLat / 2) * Math.sin(dLat / 2) +
    Math.cos(lat1 * (Math.PI / 180)) *
      Math.cos(lat2 * (Math.PI / 180)) *
      Math.sin(dLon / 2) *
      Math.sin(dLon / 2)
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a))
  return radius * c
}

const distanceLabel = (facility: FacilitySummary) => {
  if (hasKeywordQuery.value) return null
  const miles = haversineMiles(
    activeCenter.value.latitude,
    activeCenter.value.longitude,
    facility.latitude,
    facility.longitude,
  )
  return `${miles.toFixed(1)} mi`
}

async function fetchFacilities() {
  loading.value = true
  error.value = null

  const query = new URLSearchParams({ limit: '200' })
  const term = search.value.trim()

  if (term) {
    query.set('q', term)
  } else {
    query.set('latitude', String(activeCenter.value.latitude))
    query.set('longitude', String(activeCenter.value.longitude))
    query.set('radius_miles', String(radiusMiles.value))
  }

  try {
    const response = await fetch(`${apiBaseUrl}/api/v1/facilities?${query.toString()}`)
    if (!response.ok) {
      throw new Error(`Failed to fetch facilities (${response.status})`)
    }

    const payload = await response.json()
    facilities.value = payload.data ?? []
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Unexpected fetch error'
  } finally {
    loading.value = false
  }
}

async function fetchIngestionStats() {
  try {
    const response = await fetch(`${apiBaseUrl}/api/v1/system/ingestion`)
    if (!response.ok) return
    const payload = await response.json()
    ingestionStats.value = payload.data ?? null
  } catch {
    // Non-blocking metadata panel.
  }
}

async function requestBrowserLocation() {
  if (!('geolocation' in navigator)) {
    locationState.value = 'unsupported'
    locationMessage.value = 'Browser geolocation is not supported on this device.'
    return
  }

  locationState.value = 'requesting'
  locationMessage.value = 'Requesting your location...'

  await new Promise<void>((resolve) => {
    navigator.geolocation.getCurrentPosition(
      (position) => {
        userLocation.value = {
          latitude: position.coords.latitude,
          longitude: position.coords.longitude,
          accuracy: position.coords.accuracy,
        }
        locationState.value = 'granted'
        locationMessage.value = `Using browser location (±${Math.round(position.coords.accuracy)}m accuracy).`
        resolve()
      },
      () => {
        userLocation.value = null
        locationState.value = 'denied'
        locationMessage.value = 'Location permission denied. Using Los Angeles fallback center.'
        resolve()
      },
      { enableHighAccuracy: true, timeout: 10000, maximumAge: 300000 },
    )
  })

  await fetchFacilities()
}

async function resetToFallback() {
  userLocation.value = null
  locationState.value = 'fallback'
  locationMessage.value = 'Using Los Angeles fallback center.'
  await fetchFacilities()
}

onMounted(async () => {
  await Promise.all([fetchFacilities(), fetchIngestionStats()])
})
</script>

<template>
  <main class="trust-shell">
    <section class="cds--tile trust-hero">
      <p class="trust-eyebrow">Trustaraunt</p>
      <h1 class="cds--productive-heading-05">Find safer food, faster.</h1>
      <p class="cds--body-compact-02">
        Carbon-driven directory for Southern California food safety data with live Trust Scores.
      </p>

      <form class="trust-search" @submit.prevent="fetchFacilities">
        <div class="cds--form-item">
          <label class="cds--label" for="query">Search Directory</label>
          <input
            id="query"
            v-model="search"
            class="cds--text-input"
            type="text"
            placeholder="Search by name, address, ZIP, or city"
          />
        </div>
        <button class="cds--btn cds--btn--primary trust-btn" type="submit">
          Search
        </button>
      </form>

      <div class="trust-actions">
        <button
          class="cds--btn cds--btn--secondary trust-btn"
          type="button"
          :disabled="locationState === 'requesting'"
          @click="requestBrowserLocation"
        >
          {{ locationState === 'requesting' ? 'Locating…' : 'Use Browser Location' }}
        </button>
        <button class="cds--btn cds--btn--tertiary trust-btn" type="button" @click="resetToFallback">
          Use LA Fallback
        </button>
      </div>

      <p class="cds--body-compact-01 trust-note">{{ locationMessage }}</p>

      <div class="cds--form-item">
        <label class="cds--label" for="radius">Radius ({{ radiusMiles.toFixed(1) }} mi)</label>
        <input
          id="radius"
          v-model.number="radiusMiles"
          class="trust-range"
          type="range"
          min="0.5"
          max="15"
          step="0.5"
          @change="fetchFacilities"
        />
      </div>
      <p class="cds--body-compact-01 trust-note">
        {{ hasKeywordQuery ? 'Keyword mode active (radius ignored).' : `Centering near ${activeCenter.label}.` }}
      </p>
    </section>

    <section class="trust-grid">
      <article class="cds--tile trust-stat-tile">
        <p class="cds--label">Facilities Loaded</p>
        <p class="cds--productive-heading-04">
          {{ ingestionStats?.unique_facilities?.toLocaleString() ?? '0' }}
        </p>
        <p class="cds--body-compact-01 trust-note">Latest ingestion snapshot.</p>
      </article>

      <article class="cds--tile trust-stat-tile">
        <p class="cds--label">Last Ingestion</p>
        <p class="cds--body-02">{{ lastRefreshLabel }}</p>
        <p class="cds--body-compact-01 trust-note">
          Search center: {{ activeCenter.label }} · Radius: {{ radiusMiles.toFixed(1) }} mi
        </p>
      </article>
    </section>

    <section class="cds--tile">
      <header class="trust-section-head">
        <h2 class="cds--productive-heading-03">Slice The Data</h2>
        <span class="cds--tag cds--tag--outline">{{ filteredFacilities.length }} result(s)</span>
      </header>

      <div class="trust-filters">
        <div class="cds--form-item">
          <label class="cds--label" for="jurisdiction">Jurisdiction</label>
          <div class="cds--select">
            <select id="jurisdiction" v-model="jurisdictionFilter" class="cds--select-input">
              <option v-for="option in jurisdictionOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
          </div>
        </div>

        <div class="cds--form-item">
          <label class="cds--label" for="sort">Sort</label>
          <div class="cds--select">
            <select id="sort" v-model="sortMode" class="cds--select-input">
              <option value="trust_desc">Trust Score (High to Low)</option>
              <option value="recent_desc">Most Recently Inspected</option>
              <option value="name_asc">Name (A to Z)</option>
            </select>
          </div>
        </div>
      </div>

      <div class="trust-slices">
        <button class="cds--btn cds--btn--ghost trust-slice-btn" type="button" @click="scoreSlice = 'all'">
          All <span class="cds--tag cds--tag--outline">{{ facilities.length }}</span>
        </button>
        <button class="cds--btn cds--btn--ghost trust-slice-btn" type="button" @click="scoreSlice = 'elite'">
          Elite <span class="cds--tag cds--tag--green">{{ scoreSlices.elite }}</span>
        </button>
        <button class="cds--btn cds--btn--ghost trust-slice-btn" type="button" @click="scoreSlice = 'solid'">
          Solid <span class="cds--tag cds--tag--warm-gray">{{ scoreSlices.solid }}</span>
        </button>
        <button class="cds--btn cds--btn--ghost trust-slice-btn" type="button" @click="scoreSlice = 'watch'">
          Watch <span class="cds--tag cds--tag--red">{{ scoreSlices.watch }}</span>
        </button>
      </div>

      <label class="cds--checkbox-label trust-checkbox">
        <input v-model="recentOnly" class="cds--checkbox" type="checkbox" />
        <span class="cds--checkbox-label-text">Only show inspections from the last 90 days</span>
      </label>
    </section>

    <section v-if="featured" class="cds--tile">
      <p class="cds--label">Top Match In Current Slice</p>
      <h3 class="cds--productive-heading-03">{{ featured.name }}</h3>
      <p class="cds--body-02">{{ featured.address }}, {{ featured.city }} {{ featured.postal_code }}</p>
      <div class="trust-inline-tags">
        <span class="cds--tag cds--tag--outline">{{ featured.jurisdiction }}</span>
        <span class="cds--tag" :class="scoreBandMeta(featured.trust_score).className">
          {{ scoreBandMeta(featured.trust_score).label }} · {{ featured.trust_score }}
        </span>
        <span v-if="distanceLabel(featured)" class="cds--tag cds--tag--outline">{{ distanceLabel(featured) }}</span>
      </div>
      <p class="cds--body-compact-01 trust-note">Last inspection: {{ formatDate(featured.latest_inspection_at) }}</p>
    </section>

    <section class="cds--tile">
      <header class="trust-section-head">
        <h2 class="cds--productive-heading-03">Directory</h2>
        <span class="cds--tag cds--tag--outline">{{ filteredFacilities.length }} result(s)</span>
      </header>

      <p v-if="loading" class="cds--body-01">Loading latest trust scores…</p>
      <p v-else-if="error" class="trust-error">{{ error }}</p>
      <p v-else-if="filteredFacilities.length === 0" class="cds--body-01">
        No facilities matched this slice. Try a wider radius or fewer filters.
      </p>

      <ul v-else class="trust-list">
        <li v-for="facility in filteredFacilities" :key="facility.id" class="cds--tile trust-list-item">
          <div>
            <p class="cds--body-02 trust-list-title">{{ facility.name }}</p>
            <p class="cds--body-compact-02">{{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}</p>
            <div class="trust-inline-tags">
              <span class="cds--tag cds--tag--outline">{{ facility.jurisdiction }}</span>
              <span v-if="distanceLabel(facility)" class="cds--tag cds--tag--outline">{{ distanceLabel(facility) }}</span>
            </div>
            <p class="cds--body-compact-01 trust-note">Inspected {{ formatDate(facility.latest_inspection_at) }}</p>
          </div>
          <span class="cds--tag" :class="scoreBandMeta(facility.trust_score).className">
            {{ facility.trust_score }}
          </span>
        </li>
      </ul>
    </section>

    <section class="cds--tile">
      <header class="trust-section-head">
        <h2 class="cds--productive-heading-03">Data Provenance</h2>
      </header>
      <p class="cds--body-compact-02">
        Trustaraunt aggregates LA County Open Data, San Diego Socrata, Long Beach public feeds, and CPRA/LIVES imports
        across Orange County, Pasadena, Riverside, and San Bernardino.
      </p>
      <ul class="trust-connectors">
        <li v-for="connector in connectorRows" :key="connector.source" class="trust-connector-row">
          <div>
            <p class="cds--body-compact-02 trust-list-title">{{ formatSourceName(connector.source) }}</p>
            <p class="cds--body-compact-01 trust-note">
              {{ connector.fetched_records.toLocaleString() }} records fetched
            </p>
          </div>
          <span class="cds--tag" :class="connector.error ? 'cds--tag--red' : 'cds--tag--green'">
            {{ connector.error ? 'Error' : 'Healthy' }}
          </span>
        </li>
      </ul>
    </section>
  </main>
</template>
