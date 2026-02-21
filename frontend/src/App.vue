<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ThumbsDown16, ThumbsUp16 } from '@carbon/icons-vue'
import { trackEvent } from './lib/analytics'

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
  likes?: number
  dislikes?: number
  vote_score?: number
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

type SliceCounts = {
  all: number
  elite: number
  solid: number
  watch: number
}

type FacilitiesResponse = {
  data: FacilitySummary[]
  count: number
  total_count: number
  page: number
  page_size: number
  slice_counts?: SliceCounts
}

type TopPicksResponse = {
  data: FacilitySummary[]
  count: number
}

type SortMode = 'trust_desc' | 'recent_desc' | 'name_asc'
type ScoreSlice = 'all' | 'elite' | 'solid' | 'watch'
type VoteType = 'like' | 'dislike'
type LocationState = 'default' | 'requesting' | 'granted' | 'denied' | 'unsupported'
type PaginationChange = { start: number; page: number; length: number }
type GeoOptions = PositionOptions

const GEO_ERROR_PERMISSION_DENIED = 1
const GEO_ERROR_POSITION_UNAVAILABLE = 2
const GEO_ERROR_TIMEOUT = 3
const fallbackLatitude = 34.0522
const fallbackLongitude = -118.2437
const fallbackLabel = 'Downtown Los Angeles'

const facilities = ref<FacilitySummary[]>([])
const totalMatches = ref(0)
const sliceCounts = ref<SliceCounts>({ all: 0, elite: 0, solid: 0, watch: 0 })
const ingestionStats = ref<IngestionStats | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const topTenLoading = ref(false)

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
const topTenFacilities = ref<FacilitySummary[]>([])
const voteInFlight = ref<Record<string, boolean>>({})
const pageSizeChoices = [12, 24, 48]
const jurisdictionOptions = [
  { label: 'All jurisdictions', value: 'all' },
  { label: 'Los Angeles County', value: 'Los Angeles County' },
  { label: 'San Diego County', value: 'San Diego County' },
  { label: 'Orange County', value: 'Orange County' },
  { label: 'Riverside County', value: 'Riverside County' },
  { label: 'San Bernardino County', value: 'San Bernardino County' },
  { label: 'Long Beach', value: 'Long Beach' },
  { label: 'Pasadena', value: 'Pasadena' },
  { label: 'Vernon', value: 'Vernon' },
]

const resolveApiBaseUrl = () => {
  const configured = import.meta.env.VITE_API_BASE_URL
  if (configured && configured.trim().length > 0) {
    return configured
  }

  if (typeof window === 'undefined') {
    return 'http://localhost:8080'
  }

  const { origin, hostname, port } = window.location
  if (hostname === 'localhost' || hostname === '127.0.0.1') {
    return port === '8080' ? origin : 'http://localhost:8080'
  }

  return origin
}

const apiBaseUrl = resolveApiBaseUrl()

const classifyQueryTerm = (term: string) => {
  const trimmed = term.trim()
  if (!trimmed) return 'empty'
  if (/^\d{5}$/.test(trimmed)) return 'zip'
  return 'text'
}

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
const featured = computed(() => facilities.value[0])
const totalPages = computed(() => Math.max(1, Math.ceil(totalMatches.value / pageSize.value)))
const pageStart = computed(() => {
  if (totalMatches.value === 0) return 0
  return (currentPage.value - 1) * pageSize.value + 1
})
const pageEnd = computed(() =>
  Math.min((currentPage.value - 1) * pageSize.value + facilities.value.length, totalMatches.value),
)
const paginationPageSizes = computed(() => pageSizeChoices)
const topTenRanked = computed(() =>
  [...topTenFacilities.value].sort((left, right) => {
    const leftLikes = left.likes ?? 0
    const rightLikes = right.likes ?? 0
    if (leftLikes !== rightLikes) {
      return rightLikes - leftLikes
    }

    const leftScore = left.vote_score ?? leftLikes - (left.dislikes ?? 0)
    const rightScore = right.vote_score ?? rightLikes - (right.dislikes ?? 0)
    if (leftScore !== rightScore) {
      return rightScore - leftScore
    }

    return right.trust_score - left.trust_score
  }),
)

const scoreBandMeta = (score: number) => {
  if (score >= 90) return { label: 'Elite', className: 'score-chip--elite' }
  if (score >= 80) return { label: 'Solid', className: 'score-chip--solid' }
  return { label: 'Watch', className: 'score-chip--watch' }
}

watch([jurisdictionFilter, sortMode, scoreSlice, recentOnly], () => {
  trackEvent('cp_filters_changed', {
    jurisdiction: jurisdictionFilter.value,
    sort_mode: sortMode.value,
    score_slice: scoreSlice.value,
    recent_only: recentOnly.value,
  })
  void fetchFacilities(true)
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

const withVoteDefaults = (facility: FacilitySummary): FacilitySummary => {
  const likes = facility.likes ?? 0
  const dislikes = facility.dislikes ?? 0
  return {
    ...facility,
    likes,
    dislikes,
    vote_score: facility.vote_score ?? likes - dislikes,
  }
}

const applyVoteSummaryToCollections = (
  facilityId: string,
  summary: { likes: number; dislikes: number; vote_score: number },
) => {
  const patchFacility = (facility: FacilitySummary) =>
    facility.id === facilityId
      ? {
          ...facility,
          likes: summary.likes,
          dislikes: summary.dislikes,
          vote_score: summary.vote_score,
        }
      : facility

  topTenFacilities.value = topTenFacilities.value.map(patchFacility)
  facilities.value = facilities.value.map(patchFacility)
}

const isVoting = (facilityId: string) => voteInFlight.value[facilityId] === true

const submitVote = async (facilityId: string, vote: VoteType) => {
  if (isVoting(facilityId)) return

  voteInFlight.value = { ...voteInFlight.value, [facilityId]: true }
  try {
    const response = await fetch(`${apiBaseUrl}/api/v1/facilities/${encodeURIComponent(facilityId)}/vote`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ vote }),
    })
    if (!response.ok) {
      if (response.status === 429) {
        error.value = 'Vote limit reached. Please wait a minute before trying again.'
        trackEvent('cp_vote_rate_limited', {
          facility_id: facilityId,
          vote,
        })
      } else {
        trackEvent('cp_vote_failed', {
          facility_id: facilityId,
          vote,
          status_code: response.status,
        })
        throw new Error(`Vote failed (${response.status})`)
      }
      return
    }

    const payload = await response.json()
    const summary = payload?.data
    if (!summary) return
    applyVoteSummaryToCollections(facilityId, {
      likes: Number(summary.likes ?? 0),
      dislikes: Number(summary.dislikes ?? 0),
      vote_score: Number(summary.vote_score ?? 0),
    })
    trackEvent('cp_vote_submitted', {
      facility_id: facilityId,
      vote,
      likes: Number(summary.likes ?? 0),
      dislikes: Number(summary.dislikes ?? 0),
      vote_score: Number(summary.vote_score ?? 0),
    })
    void fetchTopTen()
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : 'Vote submission failed'
    error.value = message
    trackEvent('cp_vote_exception', {
      facility_id: facilityId,
      vote,
      error_message: message,
    })
  } finally {
    voteInFlight.value = { ...voteInFlight.value, [facilityId]: false }
  }
}

const isLikelyMobileSafari = () => {
  if (typeof navigator === 'undefined') return false
  const ua = navigator.userAgent
  const isiOS = /iP(hone|ad|od)/.test(ua) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
  const webkit = /WebKit/i.test(ua)
  const excluded = /CriOS|FxiOS|EdgiOS|OPiOS|DuckDuckGo/i.test(ua)
  return isiOS && webkit && !excluded
}

const geolocationPermissionHint = () => {
  if (isLikelyMobileSafari()) {
    return 'Allow Location for Safari Websites in iOS Settings, then retry.'
  }
  return 'Allow location access for this site in your browser settings, then retry.'
}

const getCurrentPosition = (options: GeoOptions) =>
  new Promise<GeolocationPosition>((resolve, reject) => {
    navigator.geolocation.getCurrentPosition(resolve, reject, options)
  })

const buildFacilitiesQuery = (page: number, requestedPageSize: number, sort: SortMode) => {
  const query = new URLSearchParams({
    page: String(page),
    page_size: String(requestedPageSize),
    sort,
  })
  const term = search.value.trim()

  if (term) {
    query.set('q', term)
  } else {
    query.set('latitude', String(activeCenter.value.latitude))
    query.set('longitude', String(activeCenter.value.longitude))
    query.set('radius_miles', String(radiusMiles.value))
  }

  if (jurisdictionFilter.value !== 'all') {
    query.set('jurisdiction', jurisdictionFilter.value)
  }

  if (scoreSlice.value !== 'all') {
    query.set('score_slice', scoreSlice.value)
  }

  if (recentOnly.value) {
    query.set('recent_only', 'true')
  }

  return query
}

async function fetchTopTen() {
  topTenLoading.value = true

  try {
    const response = await fetch(`${apiBaseUrl}/api/v1/facilities/top-picks?limit=10`)
    if (!response.ok) {
      throw new Error(`Failed to fetch top 10 (${response.status})`)
    }

    const payload: TopPicksResponse = await response.json()
    topTenFacilities.value = (payload.data ?? []).map(withVoteDefaults)
    trackEvent('cp_top10_loaded', {
      top10_count: topTenFacilities.value.length,
      top_like_count: topTenFacilities.value[0]?.likes ?? 0,
    })
  } catch {
    topTenFacilities.value = []
    trackEvent('cp_top10_load_failed')
  } finally {
    topTenLoading.value = false
  }
}

const onPaginationChange = ({ page, length }: PaginationChange) => {
  const nextPage = Math.max(1, page)
  const nextPageSize = Math.max(1, length)
  if (nextPage === currentPage.value && nextPageSize === pageSize.value) {
    return
  }

  pageSize.value = nextPageSize
  currentPage.value = nextPage
  trackEvent('cp_pagination_changed', {
    page: currentPage.value,
    page_size: pageSize.value,
    total_matches: totalMatches.value,
  })
  void fetchFacilities()
}

const onRadiusChange = (rawValue: string | number) => {
  const parsed = typeof rawValue === 'string' ? Number.parseFloat(rawValue) : rawValue
  if (Number.isFinite(parsed)) {
    radiusMiles.value = parsed
    trackEvent('cp_radius_changed', {
      radius_miles: radiusMiles.value,
      keyword_mode: hasKeywordQuery.value,
    })
  }
  void fetchFacilities(true)
}

async function fetchFacilities(resetPage = false) {
  if (resetPage) {
    currentPage.value = 1
  }

  loading.value = true
  error.value = null

  const query = buildFacilitiesQuery(currentPage.value, pageSize.value, sortMode.value)

  try {
    const response = await fetch(`${apiBaseUrl}/api/v1/facilities?${query.toString()}`)
    if (!response.ok) {
      throw new Error(`Failed to fetch facilities (${response.status})`)
    }

    const payload: FacilitiesResponse = await response.json()
    facilities.value = (payload.data ?? []).map(withVoteDefaults)
    totalMatches.value = payload.total_count ?? payload.count ?? facilities.value.length
    currentPage.value = payload.page ?? currentPage.value
    pageSize.value = payload.page_size ?? pageSize.value
    sliceCounts.value = payload.slice_counts ?? {
      all: totalMatches.value,
      elite: 0,
      solid: 0,
      watch: 0,
    }

    if (currentPage.value > totalPages.value) {
      currentPage.value = totalPages.value
      if (totalMatches.value > 0) {
        await fetchFacilities()
      }
    }
    trackEvent('cp_search_results_loaded', {
      total_count: totalMatches.value,
      page: currentPage.value,
      page_size: pageSize.value,
      result_count: facilities.value.length,
      query_type: classifyQueryTerm(search.value),
      query_length: search.value.trim().length,
      jurisdiction: jurisdictionFilter.value,
      score_slice: scoreSlice.value,
      sort_mode: sortMode.value,
      recent_only: recentOnly.value,
      radius_miles: radiusMiles.value,
      keyword_mode: hasKeywordQuery.value,
    })
    void fetchTopTen()
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Unexpected fetch error'
    trackEvent('cp_search_results_failed', {
      query_type: classifyQueryTerm(search.value),
      query_length: search.value.trim().length,
      jurisdiction: jurisdictionFilter.value,
      score_slice: scoreSlice.value,
      sort_mode: sortMode.value,
      recent_only: recentOnly.value,
      radius_miles: radiusMiles.value,
    })
  } finally {
    loading.value = false
  }
}

async function onSearchSubmit() {
  trackEvent('cp_search_submitted', {
    query_type: classifyQueryTerm(search.value),
    query_length: search.value.trim().length,
    keyword_mode: hasKeywordQuery.value,
    jurisdiction: jurisdictionFilter.value,
    score_slice: scoreSlice.value,
    recent_only: recentOnly.value,
    radius_miles: radiusMiles.value,
  })

  // Keyword searches should default to broad matching.
  if (hasKeywordQuery.value) {
    const changedFilters = scoreSlice.value !== 'all' || recentOnly.value
    scoreSlice.value = 'all'
    recentOnly.value = false
    if (changedFilters) {
      // watcher will refetch with reset filters
      return
    }
  }

  await fetchFacilities(true)
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
  if (locationState.value === 'requesting') return

  trackEvent('cp_location_requested')

  if (!window.isSecureContext) {
    locationState.value = 'unsupported'
    locationMessage.value = 'Location requires a secure HTTPS connection.'
    trackEvent('cp_location_result', { status: 'unsupported_insecure_context' })
    return
  }

  if (!('geolocation' in navigator)) {
    locationState.value = 'unsupported'
    locationMessage.value = 'Browser geolocation is not supported on this device.'
    trackEvent('cp_location_result', { status: 'unsupported_browser' })
    return
  }

  locationState.value = 'requesting'
  locationMessage.value = 'Requesting your location...'

  try {
    let position: GeolocationPosition

    try {
      position = await getCurrentPosition({
        enableHighAccuracy: true,
        timeout: 12000,
        maximumAge: 300000,
      })
    } catch (firstError) {
      const geolocationError = firstError as GeolocationPositionError
      if (
        geolocationError?.code === GEO_ERROR_TIMEOUT ||
        geolocationError?.code === GEO_ERROR_POSITION_UNAVAILABLE
      ) {
        position = await getCurrentPosition({
          enableHighAccuracy: false,
          timeout: 20000,
          maximumAge: 900000,
        })
      } else {
        throw firstError
      }
    }

    userLocation.value = {
      latitude: position.coords.latitude,
      longitude: position.coords.longitude,
      accuracy: position.coords.accuracy,
    }
    locationState.value = 'granted'
    locationMessage.value = `Using browser location (±${Math.round(position.coords.accuracy)}m accuracy).`
    trackEvent('cp_location_result', {
      status: 'granted',
      accuracy_meters: Math.round(position.coords.accuracy),
    })
  } catch (cause) {
    userLocation.value = null
    const geolocationError = cause as GeolocationPositionError

    if (geolocationError?.code === GEO_ERROR_PERMISSION_DENIED) {
      locationState.value = 'denied'
      locationMessage.value = `Location permission denied. ${geolocationPermissionHint()} Reverting to Southern California default center.`
      trackEvent('cp_location_result', { status: 'permission_denied' })
    } else if (geolocationError?.code === GEO_ERROR_TIMEOUT) {
      locationState.value = 'default'
      locationMessage.value =
        'Location lookup timed out on this network/device. Reverting to Southern California default center.'
      trackEvent('cp_location_result', { status: 'timeout' })
    } else if (geolocationError?.code === GEO_ERROR_POSITION_UNAVAILABLE) {
      locationState.value = 'default'
      locationMessage.value =
        'Location is currently unavailable on this device. Reverting to Southern California default center.'
      trackEvent('cp_location_result', { status: 'position_unavailable' })
    } else {
      locationState.value = 'default'
      locationMessage.value = 'Could not determine your location. Reverting to Southern California default center.'
      trackEvent('cp_location_result', { status: 'unknown_error' })
    }
  }

  await fetchFacilities(true)
}

onMounted(async () => {
  trackEvent('cp_app_loaded', {
    page_path: window.location.pathname,
    page_title: document.title,
  })
  await Promise.all([fetchFacilities(true), fetchIngestionStats()])
})
</script>

<template>
  <main class="trust-app">
    <section class="trust-panel trust-panel--hero">
      <p class="trust-eyebrow">CleanPlated</p>
      <h1 class="trust-title">Find safer food, faster.</h1>
      <p class="trust-lede">
        Live Southern California restaurant inspection data, normalized into one clear Trust Score.
      </p>

      <form class="trust-form" @submit.prevent="onSearchSubmit">
        <cv-text-input
          v-model="search"
          label="Search Directory"
          placeholder="Search by business, address, ZIP, or city"
          size="lg"
        >
          <template v-slot:helper-text> Search by restaurant name, address, ZIP code, or city </template>
        </cv-text-input>

        <div class="trust-actions">
          <cv-button kind="primary" type="submit">Search</cv-button>
          <cv-button
            kind="secondary"
            type="button"
            :disabled="locationState === 'requesting'"
            @click.prevent.stop="requestBrowserLocation"
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
      <cv-tile class="trust-stat-tile">
        <p class="trust-kicker">Restaurants Indexed</p>
        <p class="trust-stat">{{ ingestionStats?.unique_facilities?.toLocaleString() ?? '0' }}</p>
        <p class="trust-note">Current snapshot from the latest ingestion run.</p>
      </cv-tile>

      <cv-tile class="trust-stat-tile">
        <p class="trust-kicker">Last Ingestion</p>
        <p class="trust-stat trust-stat--small">{{ lastRefreshLabel }}</p>
        <p class="trust-note">Search center: {{ activeCenter.label }} · Radius: {{ radiusMiles.toFixed(1) }} mi</p>
      </cv-tile>
    </section>

    <section class="trust-panel">
      <header class="trust-section-head">
        <h2 class="trust-heading">Filter Results</h2>
        <cv-tag :label="`${totalMatches.toLocaleString()} result(s)`" kind="cool-gray" />
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
          All ({{ sliceCounts.all.toLocaleString() }})
        </cv-button>
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'elite' }" @click="scoreSlice = 'elite'">
          Elite ({{ sliceCounts.elite.toLocaleString() }})
        </cv-button>
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'solid' }" @click="scoreSlice = 'solid'">
          Solid ({{ sliceCounts.solid.toLocaleString() }})
        </cv-button>
        <cv-button kind="ghost" :class="{ 'slice-active': scoreSlice === 'watch' }" @click="scoreSlice = 'watch'">
          Watch ({{ sliceCounts.watch.toLocaleString() }})
        </cv-button>
      </div>

      <cv-checkbox v-model="recentOnly" label="Only show inspections from the last 90 days" />
    </section>

    <section v-if="featured" class="trust-panel">
      <p class="trust-kicker">Top match in current results</p>
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
        <h2 class="trust-heading">Top 10 Most Liked</h2>
      </header>
      <p class="trust-note">Ranked by total thumbs up across the community. Add your vote to influence this list.</p>

      <cv-inline-loading v-if="topTenLoading" state="loading" loading-text="Loading top facilities..." />
      <cv-inline-notification
        v-else-if="topTenRanked.length === 0"
        kind="info"
        title="No community likes yet"
        sub-title="Be the first to like a restaurant and shape the Top 10."
        :hide-close-button="true"
      />

      <ol v-else class="trust-topten">
        <li v-for="(facility, index) in topTenRanked" :key="facility.id" class="trust-topten__item">
          <div class="trust-topten__rank">{{ index + 1 }}</div>
          <div class="trust-topten__main">
            <p class="trust-card__title">{{ facility.name }}</p>
            <p class="trust-address">{{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}</p>
            <div class="trust-tags">
              <cv-tag :label="facility.jurisdiction" kind="cool-gray" />
              <cv-tag :label="`${facility.trust_score}`" kind="green" />
            </div>
          </div>
          <div class="trust-topten__actions">
            <cv-button
              size="sm"
              kind="ghost"
              :disabled="isVoting(facility.id)"
              @click="submitVote(facility.id, 'like')"
            >
              <ThumbsUp16 />
              <span class="trust-vote-count">{{ facility.likes ?? 0 }}</span>
            </cv-button>
            <cv-button
              size="sm"
              kind="ghost"
              :disabled="isVoting(facility.id)"
              @click="submitVote(facility.id, 'dislike')"
            >
              <ThumbsDown16 />
              <span class="trust-vote-count">{{ facility.dislikes ?? 0 }}</span>
            </cv-button>
          </div>
        </li>
      </ol>
    </section>

    <section class="trust-panel">
      <header class="trust-section-head">
        <h2 class="trust-heading">Directory</h2>
        <cv-tag :label="`${totalMatches.toLocaleString()} result(s)`" kind="cool-gray" />
      </header>

      <p v-if="totalMatches > 0" class="trust-note">
        Showing {{ pageStart.toLocaleString() }}–{{ pageEnd.toLocaleString() }} of {{ totalMatches.toLocaleString() }}
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
        v-else-if="totalMatches === 0"
        kind="info"
        title="No matching facilities"
        sub-title="Try a wider radius or fewer filters."
        :hide-close-button="true"
      />

      <ul v-else class="trust-directory">
        <li v-for="facility in facilities" :key="facility.id" class="trust-card">
          <div class="trust-card__main">
            <p class="trust-card__title">{{ facility.name }}</p>
            <p class="trust-address">{{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}</p>
            <div class="trust-tags">
              <cv-tag :label="facility.jurisdiction" kind="cool-gray" />
              <cv-tag v-if="distanceLabel(facility)" :label="distanceLabel(facility) ?? ''" kind="teal" />
            </div>
            <p class="trust-note">Inspected {{ formatDate(facility.latest_inspection_at) }}</p>
          </div>
          <div class="trust-card__actions">
            <cv-tag :label="`${facility.trust_score}`" kind="green" />
          </div>
        </li>
      </ul>

      <div v-if="totalMatches > 0" class="trust-pagination">
        <cv-pagination
          :number-of-items="totalMatches"
          :page="currentPage"
          :page-sizes="paginationPageSizes"
          :page-size="pageSize"
          @change="onPaginationChange"
        />
      </div>
    </section>

    <section class="trust-panel">
      <header class="trust-section-head">
        <h2 class="trust-heading">Data Sources</h2>
      </header>
      <p class="trust-note">
        CleanPlated combines official county and city inspection sources across Southern California,
        including LA Open Data, San Diego Socrata, Long Beach Public Health, Orange/Pasadena public records,
        and Riverside/San Bernardino LIVES feeds.
      </p>
      <cv-structured-list>
        <template v-slot:headings>
          <cv-structured-list-heading>Data Source</cv-structured-list-heading>
          <cv-structured-list-heading>Records Fetched</cv-structured-list-heading>
          <cv-structured-list-heading>Status</cv-structured-list-heading>
        </template>
        <template v-slot:items>
          <cv-structured-list-item v-for="connector in connectorRows" :key="connector.source">
            <cv-structured-list-data>
              {{ formatSourceName(connector.source) }}
              <cv-inline-notification
                v-if="connector.error"
                kind="warning"
                title="Connector issue"
                :sub-title="summarizeConnectorError(connector.error) ?? ''"
                :hide-close-button="true"
                :low-contrast="true"
                style="margin-top: 0.5rem;"
              />
            </cv-structured-list-data>
            <cv-structured-list-data>
              {{ connector.error ? 'N/A' : connector.fetched_records.toLocaleString() }}
            </cv-structured-list-data>
            <cv-structured-list-data>
              <cv-tag :label="connector.error ? 'Error' : 'Healthy'" :kind="connector.error ? 'red' : 'green'" />
            </cv-structured-list-data>
          </cv-structured-list-item>
        </template>
      </cv-structured-list>
    </section>
  </main>
</template>
