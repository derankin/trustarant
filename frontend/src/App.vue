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
type PaginationChange = { start: number; page: number; length: number }

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
const pageSizeChoices = [12, 24, 48]

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
const pageStart = computed(() => {
  if (filteredFacilities.value.length === 0) return 0
  return (currentPage.value - 1) * pageSize.value + 1
})
const pageEnd = computed(() => Math.min(currentPage.value * pageSize.value, filteredFacilities.value.length))
const paginationPageSizes = computed(() =>
  pageSizeChoices.map((value) => ({
    value,
    selected: value === pageSize.value,
  })),
)

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

const onPaginationChange = ({ page, length }: PaginationChange) => {
  currentPage.value = Math.max(1, page)
  pageSize.value = length
}

const onRadiusChange = (rawValue: string | number) => {
  const parsed = typeof rawValue === 'string' ? Number.parseFloat(rawValue) : rawValue
  if (Number.isFinite(parsed)) {
    radiusMiles.value = parsed
  }
  void fetchFacilities()
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
  <main class="trust-app">
    <section class="trust-panel trust-panel--hero">
      <p class="trust-eyebrow">Trustaraunt</p>
      <h1 class="trust-title">Find safer food, faster.</h1>
      <p class="trust-lede">Southern California food safety data, normalized into one reliable Trust Score.</p>

      <form class="trust-form" @submit.prevent="fetchFacilities">
        <cv-search
          v-model="search"
          label="Search Directory"
          placeholder="Search by business, address, ZIP, or city"
          size="lg"
        />

        <div class="trust-actions">
          <cv-button kind="primary" type="submit">Search</cv-button>
          <cv-button
            kind="secondary"
            type="button"
            :disabled="locationState === 'requesting'"
            @click="requestBrowserLocation"
          >
            {{ locationState === 'requesting' ? 'Locating…' : 'Use Browser Location' }}
          </cv-button>
        </div>
      </form>

      <p class="trust-note">{{ locationMessage }}</p>
      <p class="trust-note">
        {{ hasKeywordQuery ? 'Keyword mode active (radius ignored).' : `Centering near ${activeCenter.label}.` }}
      </p>

      <cv-slider
        label="Search Radius"
        :model-value="String(radiusMiles)"
        min="0.5"
        max="15"
        step="0.5"
        :min-label="'0.5 mi'"
        :max-label="'15 mi'"
        @change="onRadiusChange"
      />
    </section>

    <section class="trust-stats">
      <article class="trust-panel">
        <p class="trust-kicker">Facilities Loaded</p>
        <p class="trust-stat">{{ ingestionStats?.unique_facilities?.toLocaleString() ?? '0' }}</p>
        <p class="trust-note">Latest ingestion snapshot.</p>
      </article>

      <article class="trust-panel">
        <p class="trust-kicker">Last Ingestion</p>
        <p class="trust-stat trust-stat--small">{{ lastRefreshLabel }}</p>
        <p class="trust-note">Search center: {{ activeCenter.label }} · Radius: {{ radiusMiles.toFixed(1) }} mi</p>
      </article>
    </section>

    <section class="trust-panel">
      <header class="trust-section-head">
        <h2 class="trust-heading">Slice the data</h2>
        <cv-tag :label="`${filteredFacilities.length} result(s)`" kind="cool-gray" />
      </header>

      <div class="trust-filters">
        <cv-select v-model="jurisdictionFilter" label="Jurisdiction">
          <cv-select-option v-for="option in jurisdictionOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </cv-select-option>
        </cv-select>

        <cv-select v-model="sortMode" label="Sort">
          <cv-select-option value="trust_desc">Trust Score (High to Low)</cv-select-option>
          <cv-select-option value="recent_desc">Most Recently Inspected</cv-select-option>
          <cv-select-option value="name_asc">Name (A to Z)</cv-select-option>
        </cv-select>
      </div>

      <div class="trust-slices">
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'all' }" @click="scoreSlice = 'all'">
          All · {{ facilities.length }}
        </cv-button>
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'elite' }" @click="scoreSlice = 'elite'">
          Elite · {{ scoreSlices.elite }}
        </cv-button>
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'solid' }" @click="scoreSlice = 'solid'">
          Solid · {{ scoreSlices.solid }}
        </cv-button>
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'watch' }" @click="scoreSlice = 'watch'">
          Watch · {{ scoreSlices.watch }}
        </cv-button>
      </div>

      <cv-checkbox
        v-model="recentOnly"
        value="recent-only"
        label="Only show inspections from the last 90 days"
      />
    </section>

    <section v-if="featured" class="trust-panel">
      <p class="trust-kicker">Top Match In Current Slice</p>
      <h3 class="trust-subheading">{{ featured.name }}</h3>
      <p class="trust-address">{{ featured.address }}, {{ featured.city }} {{ featured.postal_code }}</p>
      <div class="trust-tags">
        <cv-tag :label="featured.jurisdiction" kind="cool-gray" />
        <cv-tag :label="`${scoreBandMeta(featured.trust_score).label} · ${featured.trust_score}`" kind="green" />
        <cv-tag v-if="distanceLabel(featured)" :label="distanceLabel(featured) ?? ''" kind="teal" />
      </div>
      <p class="trust-note">Last inspection: {{ formatDate(featured.latest_inspection_at) }}</p>
    </section>

    <section class="trust-panel">
      <header class="trust-section-head">
        <h2 class="trust-heading">Directory</h2>
        <cv-tag :label="`${filteredFacilities.length} result(s)`" kind="cool-gray" />
      </header>

      <p v-if="filteredFacilities.length > 0" class="trust-note">
        Showing {{ pageStart }}–{{ pageEnd }} of {{ filteredFacilities.length }}
      </p>

      <cv-inline-loading v-if="loading" state="loading" loading-text="Loading latest trust scores..." />
      <cv-inline-notification
        v-else-if="error"
        kind="error"
        title="Directory request failed"
        :sub-title="error"
        :hide-close-button="true"
      />
      <cv-inline-notification
        v-else-if="filteredFacilities.length === 0"
        kind="info"
        title="No matching facilities"
        sub-title="Try a wider radius or fewer filters."
        :hide-close-button="true"
      />

      <ul v-else class="trust-directory">
        <li v-for="facility in paginatedFacilities" :key="facility.id" class="trust-card">
          <div class="trust-card__main">
            <p class="trust-card__title">{{ facility.name }}</p>
            <p class="trust-address">{{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}</p>
            <div class="trust-tags">
              <cv-tag :label="facility.jurisdiction" kind="cool-gray" />
              <cv-tag v-if="distanceLabel(facility)" :label="distanceLabel(facility) ?? ''" kind="teal" />
            </div>
            <p class="trust-note">Inspected {{ formatDate(facility.latest_inspection_at) }}</p>
          </div>
          <cv-tag :label="`${facility.trust_score}`" kind="green" />
        </li>
      </ul>

      <div v-if="filteredFacilities.length > 0" class="trust-pagination">
        <cv-pagination
          :number-of-items="filteredFacilities.length"
          :actual-items-on-page="paginatedFacilities.length"
          :page="currentPage"
          :page-sizes="paginationPageSizes"
          @change="onPaginationChange"
        />
      </div>
    </section>

    <section class="trust-panel">
      <header class="trust-section-head">
        <h2 class="trust-heading">Data provenance</h2>
      </header>
      <p class="trust-note">
        Trustaraunt aggregates LA County Open Data, San Diego Socrata, Long Beach public feeds,
        and Orange/Pasadena public-record or portal data, plus Riverside/San Bernardino LIVES.
      </p>
      <ul class="trust-provenance">
        <li v-for="connector in connectorRows" :key="connector.source" class="trust-card trust-card--provenance">
          <div class="trust-card__main">
            <p class="trust-card__title">{{ formatSourceName(connector.source) }}</p>
            <p class="trust-note">
              {{
                connector.error
                  ? 'Unavailable in latest ingestion run'
                  : `${connector.fetched_records.toLocaleString()} records fetched`
              }}
            </p>
            <cv-inline-notification
              v-if="connector.error"
              kind="warning"
              title="Connector warning"
              :sub-title="summarizeConnectorError(connector.error) ?? ''"
              :hide-close-button="true"
              :low-contrast="true"
            />
          </div>
          <cv-tag :label="connector.error ? 'Error' : 'Healthy'" :kind="connector.error ? 'red' : 'green'" />
        </li>
      </ul>
    </section>
  </main>
</template>
