<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

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
type LocationState = 'default' | 'requesting' | 'granted' | 'denied' | 'unsupported'

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
const locationState = ref<LocationState>('default')
const locationMessage = ref('Using Southern California default center (Downtown Los Angeles).')

const currentPage = ref(1)
const pageSize = ref(12)

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8080'

const activeCenter = computed(() => {
  if (userLocation.value) {
    return {
      latitude: userLocation.value.latitude,
      longitude: userLocation.value.longitude,
      label: 'Your current location',
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
  if (score >= 90) return { label: 'Elite', className: 'score-chip--elite' }
  if (score >= 80) return { label: 'Solid', className: 'score-chip--solid' }
  return { label: 'Watch', className: 'score-chip--watch' }
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

const totalPages = computed(() => Math.max(1, Math.ceil(filteredFacilities.value.length / pageSize.value)))
const paginatedFacilities = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  return filteredFacilities.value.slice(start, start + pageSize.value)
})
const pageWindow = computed(() => {
  const pages: number[] = []
  const maxVisible = 5
  const half = Math.floor(maxVisible / 2)
  let start = Math.max(1, currentPage.value - half)
  let end = Math.min(totalPages.value, start + maxVisible - 1)

  if (end - start + 1 < maxVisible) {
    start = Math.max(1, end - maxVisible + 1)
  }

  for (let page = start; page <= end; page += 1) {
    pages.push(page)
  }
  return pages
})
const pageStart = computed(() => {
  if (filteredFacilities.value.length === 0) return 0
  return (currentPage.value - 1) * pageSize.value + 1
})
const pageEnd = computed(() => Math.min(currentPage.value * pageSize.value, filteredFacilities.value.length))

watch(
  filteredFacilities,
  () => {
    if (currentPage.value > totalPages.value) {
      currentPage.value = totalPages.value
    }
  },
  { immediate: true },
)

watch([jurisdictionFilter, sortMode, scoreSlice, recentOnly], () => {
  currentPage.value = 1
})

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
    cpra_import_orange_pasadena: 'Orange County + Pasadena Public Records/Portal',
  }

  return labels[source] ?? source.replace(/_/g, ' ')
}

const summarizeConnectorError = (value?: string | null) => {
  if (!value) return null
  const firstLine = value.split('\n')[0] ?? value
  return firstLine.slice(0, 180)
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

const goToPage = (page: number) => {
  currentPage.value = Math.min(totalPages.value, Math.max(1, page))
}

async function fetchFacilities() {
  loading.value = true
  error.value = null
  currentPage.value = 1

  const query = new URLSearchParams({ limit: '1000' })
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
        locationMessage.value = 'Location permission denied. Reverting to Southern California default center.'
        resolve()
      },
      { enableHighAccuracy: true, timeout: 10000, maximumAge: 300000 },
    )
  })

  await fetchFacilities()
}

onMounted(async () => {
  await Promise.all([fetchFacilities(), fetchIngestionStats()])
})
</script>

<template>
  <main class="trust-shell">
    <section class="panel panel--hero">
      <p class="eyebrow">Trustaraunt</p>
      <h1>Find safer food, faster.</h1>
      <p class="lede">Southern California food safety data, normalized into one reliable Trust Score.</p>

      <form class="search-row" @submit.prevent="fetchFacilities">
        <label class="field-label" for="query">Search Directory</label>
        <div class="search-controls">
          <input
            id="query"
            v-model="search"
            class="text-input"
            type="text"
            placeholder="Search by business, address, ZIP, or city"
          />
          <button class="btn btn--primary" type="submit">Search</button>
        </div>
      </form>

      <div class="action-row">
        <button
          class="btn btn--secondary"
          type="button"
          :disabled="locationState === 'requesting'"
          @click="requestBrowserLocation"
        >
          {{ locationState === 'requesting' ? 'Locating…' : 'Use Browser Location' }}
        </button>
      </div>

      <p class="note">{{ locationMessage }}</p>

      <div class="range-wrap">
        <label class="field-label" for="radius">Radius ({{ radiusMiles.toFixed(1) }} mi)</label>
        <input
          id="radius"
          v-model.number="radiusMiles"
          class="range-input"
          type="range"
          min="0.5"
          max="15"
          step="0.5"
          @change="fetchFacilities"
        />
      </div>
      <p class="note">
        {{ hasKeywordQuery ? 'Keyword mode active (radius ignored).' : `Centering near ${activeCenter.label}.` }}
      </p>
    </section>

    <section class="stats-grid">
      <article class="panel">
        <p class="stat-label">Facilities Loaded</p>
        <p class="stat-value">{{ ingestionStats?.unique_facilities?.toLocaleString() ?? '0' }}</p>
        <p class="note">Latest ingestion snapshot.</p>
      </article>

      <article class="panel">
        <p class="stat-label">Last Ingestion</p>
        <p class="stat-date">{{ lastRefreshLabel }}</p>
        <p class="note">Search center: {{ activeCenter.label }} · Radius: {{ radiusMiles.toFixed(1) }} mi</p>
      </article>
    </section>

    <section class="panel">
      <header class="section-head">
        <h2>Slice The Data</h2>
        <span class="badge badge--muted">{{ filteredFacilities.length }} result(s)</span>
      </header>

      <div class="filter-grid">
        <div>
          <label class="field-label" for="jurisdiction">Jurisdiction</label>
          <select id="jurisdiction" v-model="jurisdictionFilter" class="select-input">
            <option v-for="option in jurisdictionOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </div>

        <div>
          <label class="field-label" for="sort">Sort</label>
          <select id="sort" v-model="sortMode" class="select-input">
            <option value="trust_desc">Trust Score (High to Low)</option>
            <option value="recent_desc">Most Recently Inspected</option>
            <option value="name_asc">Name (A to Z)</option>
          </select>
        </div>
      </div>

      <div class="slice-grid">
        <button class="slice-btn" :class="{ 'slice-btn--active': scoreSlice === 'all' }" type="button" @click="scoreSlice = 'all'">
          <span>All</span>
          <span class="badge badge--muted">{{ facilities.length }}</span>
        </button>
        <button class="slice-btn" :class="{ 'slice-btn--active': scoreSlice === 'elite' }" type="button" @click="scoreSlice = 'elite'">
          <span>Elite</span>
          <span class="badge badge--elite">{{ scoreSlices.elite }}</span>
        </button>
        <button class="slice-btn" :class="{ 'slice-btn--active': scoreSlice === 'solid' }" type="button" @click="scoreSlice = 'solid'">
          <span>Solid</span>
          <span class="badge badge--solid">{{ scoreSlices.solid }}</span>
        </button>
        <button class="slice-btn" :class="{ 'slice-btn--active': scoreSlice === 'watch' }" type="button" @click="scoreSlice = 'watch'">
          <span>Watch</span>
          <span class="badge badge--watch">{{ scoreSlices.watch }}</span>
        </button>
      </div>

      <label class="checkbox-row" for="recent-only">
        <input id="recent-only" v-model="recentOnly" class="checkbox-input" type="checkbox" />
        <span>Only show inspections from the last 90 days</span>
      </label>
    </section>

    <section v-if="featured" class="panel">
      <p class="stat-label">Top Match In Current Slice</p>
      <h3>{{ featured.name }}</h3>
      <p class="card-address">{{ featured.address }}, {{ featured.city }} {{ featured.postal_code }}</p>
      <div class="chip-row">
        <span class="badge badge--muted">{{ featured.jurisdiction }}</span>
        <span class="badge" :class="scoreBandMeta(featured.trust_score).className">
          {{ scoreBandMeta(featured.trust_score).label }} · {{ featured.trust_score }}
        </span>
        <span v-if="distanceLabel(featured)" class="badge badge--muted">{{ distanceLabel(featured) }}</span>
      </div>
      <p class="note">Last inspection: {{ formatDate(featured.latest_inspection_at) }}</p>
    </section>

    <section class="panel">
      <header class="section-head">
        <h2>Directory</h2>
        <span class="badge badge--muted">{{ filteredFacilities.length }} result(s)</span>
      </header>

      <p class="note" v-if="filteredFacilities.length > 0">
        Showing {{ pageStart }}–{{ pageEnd }} of {{ filteredFacilities.length }}
      </p>

      <p v-if="loading" class="status-text">Loading latest trust scores…</p>
      <p v-else-if="error" class="status-text status-text--error">{{ error }}</p>
      <p v-else-if="filteredFacilities.length === 0" class="status-text">
        No facilities matched this slice. Try a wider radius or fewer filters.
      </p>

      <ul v-else class="directory-list">
        <li v-for="facility in paginatedFacilities" :key="facility.id" class="directory-item">
          <div class="directory-main">
            <p class="directory-title">{{ facility.name }}</p>
            <p class="card-address">{{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}</p>
            <div class="chip-row">
              <span class="badge badge--muted">{{ facility.jurisdiction }}</span>
              <span v-if="distanceLabel(facility)" class="badge badge--muted">{{ distanceLabel(facility) }}</span>
            </div>
            <p class="note">Inspected {{ formatDate(facility.latest_inspection_at) }}</p>
          </div>
          <span class="badge score-pill" :class="scoreBandMeta(facility.trust_score).className">
            {{ facility.trust_score }}
          </span>
        </li>
      </ul>

      <div v-if="filteredFacilities.length > pageSize" class="pagination">
        <button class="btn btn--secondary" type="button" :disabled="currentPage === 1" @click="goToPage(currentPage - 1)">
          Previous
        </button>
        <div class="page-buttons">
          <button
            v-for="page in pageWindow"
            :key="page"
            class="page-btn"
            :class="{ 'page-btn--active': page === currentPage }"
            type="button"
            @click="goToPage(page)"
          >
            {{ page }}
          </button>
        </div>
        <button class="btn btn--secondary" type="button" :disabled="currentPage === totalPages" @click="goToPage(currentPage + 1)">
          Next
        </button>
      </div>
    </section>

    <section class="panel">
      <header class="section-head">
        <h2>Data Provenance</h2>
      </header>
      <p class="note">
        Trustaraunt aggregates LA County Open Data, San Diego Socrata, Long Beach public feeds, and Orange/Pasadena public-record or portal data, plus Riverside/San Bernardino LIVES.
      </p>
      <ul class="provenance-list">
        <li v-for="connector in connectorRows" :key="connector.source" class="provenance-row">
          <div>
            <p class="directory-title">{{ formatSourceName(connector.source) }}</p>
            <p class="note">
              {{ connector.error ? 'Unavailable in latest ingestion run' : `${connector.fetched_records.toLocaleString()} records fetched` }}
            </p>
            <p v-if="connector.error" class="status-text status-text--error">
              {{ summarizeConnectorError(connector.error) }}
            </p>
          </div>
          <span class="badge" :class="connector.error ? 'badge--watch' : 'badge--elite'">
            {{ connector.error ? 'Error' : 'Healthy' }}
          </span>
        </li>
      </ul>
    </section>
  </main>
</template>
