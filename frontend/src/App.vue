<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import {
  ThumbsDown16,
  ThumbsUp16,
  Search16,
  LocationCurrent16,
  Map16,
  List16,
  Restaurant16,
  Filter16,
  ChevronLeft16,
  ChevronRight16,
  Renew16,
  Star16,
  StarFilled16,
  Information16,
  CheckmarkFilled16,
  WarningAltFilled16,
  Location16,
} from '@carbon/icons-vue'
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
type GeoOptions = PositionOptions
type ViewMode = 'list' | 'map'

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
const viewMode = ref<ViewMode>('list')
const filtersExpanded = ref(false)

const userLocation = ref<{ latitude: number; longitude: number; accuracy: number } | null>(null)
const locationState = ref<LocationState>('default')
const locationMessage = ref('Browsing near Downtown Los Angeles')

const currentPage = ref(1)
const pageSize = ref(12)
const topTenFacilities = ref<FacilitySummary[]>([])
const voteInFlight = ref<Record<string, boolean>>({})
const jurisdictionOptions = [
  { label: 'All areas', value: 'all' },
  { label: 'Los Angeles County', value: 'Los Angeles County' },
  { label: 'San Diego County', value: 'San Diego County' },
  { label: 'Orange County', value: 'Orange County' },
  { label: 'Riverside County', value: 'Riverside County' },
  { label: 'San Bernardino County', value: 'San Bernardino County' },
  { label: 'Long Beach', value: 'Long Beach' },
  { label: 'Pasadena', value: 'Pasadena' },
  { label: 'Vernon', value: 'Vernon' },
]

const googleMapsApiKey = (import.meta.env.VITE_GOOGLE_MAPS_API_KEY as string) || ''
const mapReady = ref(false)
const mapContainerRef = ref<HTMLElement | null>(null)
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let mapInstance: any = null
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let mapMarkers: any[] = []
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let mapInfoWindow: any = null

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
      label: 'your location',
    }
  }

  return {
    latitude: fallbackLatitude,
    longitude: fallbackLongitude,
    label: fallbackLabel,
  }
})

const hasKeywordQuery = computed(() => search.value.trim().length > 0)
const totalPages = computed(() => Math.max(1, Math.ceil(totalMatches.value / pageSize.value)))
const pageStart = computed(() => {
  if (totalMatches.value === 0) return 0
  return (currentPage.value - 1) * pageSize.value + 1
})
const pageEnd = computed(() =>
  Math.min((currentPage.value - 1) * pageSize.value + facilities.value.length, totalMatches.value),
)

const resultsSummary = computed(() => {
  if (loading.value) return ''
  if (totalMatches.value === 0) return 'No restaurants found'
  const noun = totalMatches.value === 1 ? 'restaurant' : 'restaurants'
  return `${totalMatches.value.toLocaleString()} ${noun} found`
})

const activeFilterCount = computed(() => {
  let count = 0
  if (jurisdictionFilter.value !== 'all') count++
  if (scoreSlice.value !== 'all') count++
  if (recentOnly.value) count++
  if (sortMode.value !== 'trust_desc') count++
  return count
})

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

const scoreColor = (score: number) => {
  if (score >= 90) return 'score--excellent'
  if (score >= 80) return 'score--good'
  return 'score--needs-attention'
}

const scoreLabel = (score: number) => {
  if (score >= 90) return 'Excellent'
  if (score >= 80) return 'Good'
  return 'Needs attention'
}

const scoreBandMeta = (score: number) => {
  if (score >= 90) return { label: 'Excellent', className: 'score-chip--elite' }
  if (score >= 80) return { label: 'Good', className: 'score-chip--solid' }
  return { label: 'Needs attention', className: 'score-chip--watch' }
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
  if (!ingestionStats.value?.last_refresh_at) return 'Updating...'
  return new Date(ingestionStats.value.last_refresh_at).toLocaleString()
})

const connectorRows = computed(() => ingestionStats.value?.connector_stats ?? [])

const formatSourceName = (source: string) => {
  const labels: Record<string, string> = {
    la_county_open_data: 'Los Angeles County',
    san_diego_socrata: 'San Diego County',
    long_beach_closures_page: 'City of Long Beach',
    lives_batch_riv_sbc: 'Riverside & San Bernardino',
    cpra_import_orange_pasadena: 'Orange County & Pasadena',
  }

  return labels[source] ?? source.replace(/_/g, ' ')
}


const formatDate = (value?: string) => {
  if (!value) return 'Not yet inspected'
  return new Date(value).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
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
        error.value = 'You\'re voting too quickly — hang tight and try again in a moment.'
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
    const message = cause instanceof Error ? cause.message : 'Something went wrong — please try again.'
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
    return 'You can enable it in iOS Settings → Safari → Location.'
  }
  return 'You can enable it in your browser\'s site settings.'
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

const goToPage = (page: number) => {
  if (page < 1 || page > totalPages.value || page === currentPage.value) return
  currentPage.value = page
  trackEvent('cp_pagination_changed', {
    page: currentPage.value,
    page_size: pageSize.value,
    total_matches: totalMatches.value,
  })
  void fetchFacilities().then(() => {
    nextTick(() => {
      document.getElementById('results-section')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
    })
  })
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
      throw new Error('We couldn\'t load restaurants right now. Please try again.')
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
    error.value = cause instanceof Error ? cause.message : 'Something unexpected happened. Please try again.'
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
    // Non-blocking metadata.
  }
}

async function requestBrowserLocation() {
  if (locationState.value === 'requesting') return

  trackEvent('cp_location_requested')

  if (!window.isSecureContext) {
    locationState.value = 'unsupported'
    locationMessage.value = 'Location needs a secure (HTTPS) connection.'
    trackEvent('cp_location_result', { status: 'unsupported_insecure_context' })
    return
  }

  if (!('geolocation' in navigator)) {
    locationState.value = 'unsupported'
    locationMessage.value = 'Location isn\'t available on this device.'
    trackEvent('cp_location_result', { status: 'unsupported_browser' })
    return
  }

  locationState.value = 'requesting'
  locationMessage.value = 'Finding your location...'

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
    locationMessage.value = 'Showing restaurants near you'
    trackEvent('cp_location_result', {
      status: 'granted',
      accuracy_meters: Math.round(position.coords.accuracy),
    })
  } catch (cause) {
    userLocation.value = null
    const geolocationError = cause as GeolocationPositionError

    if (geolocationError?.code === GEO_ERROR_PERMISSION_DENIED) {
      locationState.value = 'denied'
      locationMessage.value = `Location access wasn't granted. ${geolocationPermissionHint()} Showing Downtown LA instead.`
      trackEvent('cp_location_result', { status: 'permission_denied' })
    } else if (geolocationError?.code === GEO_ERROR_TIMEOUT) {
      locationState.value = 'default'
      locationMessage.value = 'Location took too long — showing Downtown LA instead.'
      trackEvent('cp_location_result', { status: 'timeout' })
    } else if (geolocationError?.code === GEO_ERROR_POSITION_UNAVAILABLE) {
      locationState.value = 'default'
      locationMessage.value = 'Couldn\'t determine your location right now. Showing Downtown LA instead.'
      trackEvent('cp_location_result', { status: 'position_unavailable' })
    } else {
      locationState.value = 'default'
      locationMessage.value = 'Couldn\'t get your location. Showing Downtown LA instead.'
      trackEvent('cp_location_result', { status: 'unknown_error' })
    }
  }

  await fetchFacilities(true)
}

const loadGoogleMapsScript = (): Promise<void> =>
  new Promise((resolve, reject) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if ((window as any).google?.maps) {
      resolve()
      return
    }
    const script = document.createElement('script')
    script.src = `https://maps.googleapis.com/maps/api/js?key=${googleMapsApiKey}`
    script.async = true
    script.defer = true
    script.onload = () => resolve()
    script.onerror = () => reject(new Error('Failed to load Google Maps'))
    document.head.appendChild(script)
  })

const clearMapMarkers = () => {
  for (const marker of mapMarkers) {
    marker.setMap(null)
  }
  mapMarkers = []
}

const markerColorForScore = (score: number) => {
  if (score >= 90) return '#24a148' // green
  if (score >= 80) return '#f1c21b' // yellow
  return '#da1e28' // red
}

const updateMapMarkers = () => {
  if (!mapInstance) return
  clearMapMarkers()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const g = (window as any).google
  if (mapInfoWindow) mapInfoWindow.close()
  mapInfoWindow = new g.maps.InfoWindow()
  const bounds = new g.maps.LatLngBounds()
  let hasCoords = false
  for (const f of facilities.value) {
    if (!f.latitude || !f.longitude) continue
    hasCoords = true
    const pos = { lat: f.latitude, lng: f.longitude }
    const pinColor = markerColorForScore(f.trust_score)
    const marker = new g.maps.Marker({
      position: pos,
      map: mapInstance,
      title: f.name,
      icon: {
        path: g.maps.SymbolPath.CIRCLE,
        scale: 8,
        fillColor: pinColor,
        fillOpacity: 0.9,
        strokeColor: '#fff',
        strokeWeight: 2,
      },
    })
    marker.addListener('click', () => {
      const band = scoreBandMeta(f.trust_score)
      mapInfoWindow.setContent(
        `<div style="font-family:IBM Plex Sans,sans-serif;max-width:220px;padding:4px 0">` +
        `<strong style="font-size:14px">${f.name}</strong>` +
        `<div style="color:#525252;font-size:12px;margin-top:2px">${f.address}, ${f.city}</div>` +
        `<div style="margin-top:6px;display:inline-flex;align-items:center;gap:6px">` +
        `<span style="background:${pinColor};color:#fff;padding:2px 8px;border-radius:12px;font-size:12px;font-weight:600">${f.trust_score}</span>` +
        `<span style="font-size:12px;color:#525252">${band.label}</span>` +
        `</div></div>`
      )
      mapInfoWindow.open(mapInstance, marker)
    })
    mapMarkers.push(marker)
    bounds.extend(pos)
  }
  if (hasCoords) {
    mapInstance.fitBounds(bounds)
    if (mapMarkers.length === 1) mapInstance.setZoom(15)
  }
}

const initializeMap = async () => {
  if (!googleMapsApiKey || !mapReady.value || !mapContainerRef.value) return
  if (mapInstance) {
    updateMapMarkers()
    return
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const g = (window as any).google
  mapInstance = new g.maps.Map(mapContainerRef.value, {
    center: { lat: activeCenter.value.latitude, lng: activeCenter.value.longitude },
    zoom: 12,
    mapTypeControl: false,
    streetViewControl: false,
    fullscreenControl: false,
    styles: [
      { featureType: 'poi', stylers: [{ visibility: 'off' }] },
      { featureType: 'transit', stylers: [{ visibility: 'simplified' }] },
    ],
  })
  updateMapMarkers()
}

const switchView = async (mode: ViewMode) => {
  viewMode.value = mode
  trackEvent(mode === 'map' ? 'cp_map_expanded' : 'cp_map_collapsed')
  if (mode === 'map') {
    await nextTick()
    if (!mapInstance) {
      await initializeMap()
    } else {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const g = (window as any).google
      g?.maps?.event?.trigger(mapInstance, 'resize')
      updateMapMarkers()
    }
  }
}

watch(facilities, () => {
  if (viewMode.value === 'map' && mapInstance) {
    updateMapMarkers()
  }
})

onMounted(async () => {
  trackEvent('cp_app_loaded', {
    page_path: window.location.pathname,
    page_title: document.title,
  })
  const startupTasks: Promise<void>[] = [fetchFacilities(true), fetchIngestionStats()]
  if (googleMapsApiKey) {
    startupTasks.push(
      loadGoogleMapsScript()
        .then(() => { mapReady.value = true })
        .catch(() => { /* map is optional */ })
    )
  }
  await Promise.all(startupTasks)
})
</script>

<template>
  <main class="cp-app">
    <!-- ─── Hero ─── -->
    <section class="cp-hero">
      <div class="cp-hero__logo">
        <img src="/cleanplated-logo.svg" alt="CleanPlated" width="72" height="72" />
      </div>
      <div class="cp-hero__content">
        <p class="cp-hero__eyebrow">CleanPlated</p>
        <h1 class="cp-hero__title">Know before you go.</h1>
        <p class="cp-hero__lede">
          Real inspection data from across Southern California — so you can
          dine with confidence and discover top-rated restaurants.
        </p>
      </div>

      <!-- ─── Search bar ─── -->
      <form class="cp-search" @submit.prevent="onSearchSubmit">
        <cv-search
          v-model="search"
          size="lg"
          placeholder="Restaurant name, address, or ZIP"
          label="Search restaurants"
          :form-item="false"
          class="cp-search__input"
        />
        <div class="cp-search__actions">
          <cv-button kind="primary" :icon="Search16" @click="onSearchSubmit">
            Search
          </cv-button>
          <cv-button
            kind="tertiary"
            :icon="Location16"
            :disabled="locationState === 'requesting'"
            @click="requestBrowserLocation"
          >
            {{ locationState === 'requesting' ? 'Finding...' : 'Near me' }}
          </cv-button>
        </div>
        <p class="cp-search__context">
          <LocationCurrent16 class="cp-search__context-icon" />
          {{ locationMessage }}
        </p>
      </form>
    </section>

    <!-- ─── Quick stats ─── -->
    <section class="cp-stats">
      <div class="cp-stat">
        <span class="cp-stat__value">{{ ingestionStats?.unique_facilities?.toLocaleString() ?? '—' }}</span>
        <span class="cp-stat__label">Restaurants</span>
      </div>
      <div class="cp-stat">
        <span class="cp-stat__value">{{ sliceCounts.elite.toLocaleString() }}</span>
        <span class="cp-stat__label">Excellent rated</span>
      </div>
      <div class="cp-stat">
        <span class="cp-stat__value">{{ radiusMiles.toFixed(0) }} mi</span>
        <span class="cp-stat__label">Search radius</span>
      </div>
    </section>

    <!-- ─── Radius slider ─── -->
    <section v-if="!hasKeywordQuery" class="cp-panel cp-radius">
      <cv-slider
        label="Search radius"
        :model-value="String(radiusMiles)"
        min="0.5"
        max="15"
        step="0.5"
        :min-label="'0.5 mi'"
        :max-label="'15 mi'"
        @change="onRadiusChange"
      />
    </section>

    <!-- ─── Filters ─── -->
    <section class="cp-panel cp-filters-panel">
      <button class="cp-filters-toggle" @click="filtersExpanded = !filtersExpanded">
        <Filter16 />
        <span>Filters</span>
        <cv-tag v-if="activeFilterCount > 0" :label="String(activeFilterCount)" kind="green" />
        <ChevronRight16 :class="['cp-filters-chevron', { 'cp-filters-chevron--open': filtersExpanded }]" />
      </button>

      <div v-show="filtersExpanded" class="cp-filters-body">
        <cv-select v-model="jurisdictionFilter" label="Area" hide-label>
          <cv-select-option v-for="option in jurisdictionOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </cv-select-option>
        </cv-select>

        <cv-select v-model="sortMode" label="Sort by" hide-label>
          <cv-select-option value="trust_desc">Highest rated</cv-select-option>
          <cv-select-option value="recent_desc">Recently inspected</cv-select-option>
          <cv-select-option value="name_asc">Name A–Z</cv-select-option>
        </cv-select>

        <div class="cp-slices">
          <button
            v-for="slice in (['all', 'elite', 'solid', 'watch'] as ScoreSlice[])"
            :key="slice"
            class="cp-slice"
            :class="{ 'cp-slice--active': scoreSlice === slice }"
            @click="scoreSlice = slice"
          >
            {{ slice === 'all' ? 'All' : slice === 'elite' ? 'Excellent' : slice === 'solid' ? 'Good' : 'Needs attention' }}
            <span class="cp-slice__count">{{ (sliceCounts as Record<string, number>)[slice]?.toLocaleString() ?? '0' }}</span>
          </button>
        </div>

        <cv-checkbox v-model="recentOnly" label="Inspected in the last 90 days" />
      </div>
    </section>

    <!-- ─── Results header ─── -->
    <section id="results-section" class="cp-results-header">
      <div class="cp-results-header__summary">
        <h2 class="cp-results-header__count" v-if="!loading">{{ resultsSummary }}</h2>
        <div v-else class="cp-skeleton cp-skeleton--text" style="width:140px;height:24px"></div>
        <p class="cp-results-header__range" v-if="totalMatches > 0 && !loading">
          Showing {{ pageStart }}–{{ pageEnd }}
        </p>
      </div>
      <div class="cp-view-toggle" v-if="googleMapsApiKey">
        <button
          class="cp-view-btn"
          :class="{ 'cp-view-btn--active': viewMode === 'list' }"
          @click="switchView('list')"
          aria-label="List view"
        >
          <List16 />
        </button>
        <button
          class="cp-view-btn"
          :class="{ 'cp-view-btn--active': viewMode === 'map' }"
          @click="switchView('map')"
          aria-label="Map view"
        >
          <Map16 />
        </button>
      </div>
    </section>

    <!-- ─── Map view ─── -->
    <section v-if="viewMode === 'map' && googleMapsApiKey" class="cp-map-section">
      <div ref="mapContainerRef" class="cp-map"></div>
    </section>

    <!-- ─── Results list ─── -->
    <section v-if="viewMode === 'list'" class="cp-results">
      <!-- Loading skeletons -->
      <template v-if="loading">
        <div v-for="n in 4" :key="n" class="cp-card cp-card--skeleton">
          <div class="cp-card__body">
            <div class="cp-skeleton cp-skeleton--text" style="width:70%;height:16px"></div>
            <div class="cp-skeleton cp-skeleton--text" style="width:90%;height:12px;margin-top:8px"></div>
            <div class="cp-skeleton cp-skeleton--text" style="width:40%;height:12px;margin-top:8px"></div>
          </div>
          <div class="cp-skeleton cp-skeleton--circle"></div>
        </div>
      </template>

      <!-- Error state -->
      <div v-else-if="error" class="cp-empty">
        <WarningAltFilled16 class="cp-empty__icon cp-empty__icon--error" />
        <p class="cp-empty__title">Something went wrong</p>
        <p class="cp-empty__subtitle">{{ error }}</p>
        <cv-button kind="secondary" :icon="Renew16" @click="fetchFacilities(true)">
          Try again
        </cv-button>
      </div>

      <!-- Empty state -->
      <div v-else-if="totalMatches === 0 && !loading" class="cp-empty">
        <Restaurant16 class="cp-empty__icon" />
        <p class="cp-empty__title">No restaurants match your search</p>
        <p class="cp-empty__subtitle">
          Try expanding your search radius, removing filters, or searching by name or ZIP code.
        </p>
      </div>

      <!-- Restaurant cards -->
      <template v-else>
        <div
          v-for="facility in facilities"
          :key="facility.id"
          class="cp-card"
        >
          <div class="cp-card__body">
            <h3 class="cp-card__name">{{ facility.name }}</h3>
            <p class="cp-card__address">{{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}</p>
            <div class="cp-card__meta">
              <span class="cp-card__meta-item">{{ facility.jurisdiction }}</span>
              <span v-if="distanceLabel(facility)" class="cp-card__meta-item">{{ distanceLabel(facility) }}</span>
              <span class="cp-card__meta-item">Inspected {{ formatDate(facility.latest_inspection_at) }}</span>
            </div>
          </div>
          <div class="cp-card__score" :class="scoreColor(facility.trust_score)">
            <span class="cp-card__score-value">{{ facility.trust_score }}</span>
            <span class="cp-card__score-label">{{ scoreLabel(facility.trust_score) }}</span>
          </div>
          <div class="cp-card__votes">
            <button
              class="cp-vote-btn"
              :aria-label="`Recommend ${facility.name}`"
              :disabled="isVoting(facility.id)"
              @click="submitVote(facility.id, 'like')"
            >
              <ThumbsUp16 /> {{ facility.likes ?? 0 }}
            </button>
            <button
              class="cp-vote-btn"
              :aria-label="`Not recommended ${facility.name}`"
              :disabled="isVoting(facility.id)"
              @click="submitVote(facility.id, 'dislike')"
            >
              <ThumbsDown16 /> {{ facility.dislikes ?? 0 }}
            </button>
          </div>
        </div>
      </template>
    </section>

    <!-- ─── Pagination ─── -->
    <section v-if="totalMatches > 0 && viewMode === 'list' && !loading" class="cp-pagination">
      <p class="cp-pagination__info">
        Page {{ currentPage }} of {{ totalPages }} · {{ pageSize }} per page
      </p>
      <div class="cp-pagination__controls">
        <button
          class="cp-page-btn"
          :disabled="currentPage <= 1"
          @click="goToPage(currentPage - 1)"
          aria-label="Previous page"
        >
          <ChevronLeft16 />
        </button>
        <button
          v-for="p in Math.min(totalPages, 5)"
          :key="p"
          class="cp-page-btn"
          :class="{ 'cp-page-btn--active': p === currentPage }"
          @click="goToPage(p)"
        >
          {{ p }}
        </button>
        <span v-if="totalPages > 5" class="cp-pagination__ellipsis">…</span>
        <button
          v-if="totalPages > 5"
          class="cp-page-btn"
          :class="{ 'cp-page-btn--active': totalPages === currentPage }"
          @click="goToPage(totalPages)"
        >
          {{ totalPages }}
        </button>
        <button
          class="cp-page-btn"
          :disabled="currentPage >= totalPages"
          @click="goToPage(currentPage + 1)"
          aria-label="Next page"
        >
          <ChevronRight16 />
        </button>
      </div>
    </section>

    <!-- ─── Community favorites ─── -->
    <section class="cp-panel cp-favorites">
      <header class="cp-section-head">
        <h2 class="cp-section-title">
          <StarFilled16 class="cp-section-icon" /> Community Favorites
        </h2>
      </header>
      <p class="cp-section-desc">
        The most recommended restaurants by the CleanPlated community. Your votes shape this list.
      </p>

      <!-- Loading skeletons -->
      <template v-if="topTenLoading">
        <div v-for="n in 3" :key="n" class="cp-fav cp-fav--skeleton">
          <div class="cp-skeleton cp-skeleton--circle-sm"></div>
          <div style="flex:1">
            <div class="cp-skeleton cp-skeleton--text" style="width:65%;height:14px"></div>
            <div class="cp-skeleton cp-skeleton--text" style="width:85%;height:11px;margin-top:6px"></div>
          </div>
        </div>
      </template>

      <!-- Empty -->
      <div v-else-if="topTenRanked.length === 0" class="cp-empty cp-empty--compact">
        <Star16 class="cp-empty__icon" />
        <p class="cp-empty__title">No favorites yet</p>
        <p class="cp-empty__subtitle">Be the first to recommend a restaurant you love.</p>
      </div>

      <!-- List -->
      <ol v-else class="cp-fav-list">
        <li v-for="(facility, index) in topTenRanked" :key="facility.id" class="cp-fav">
          <div class="cp-fav__rank">{{ index + 1 }}</div>
          <div class="cp-fav__body">
            <p class="cp-fav__name">{{ facility.name }}</p>
            <p class="cp-fav__address">{{ facility.address }}, {{ facility.city }}</p>
            <div class="cp-fav__tags">
              <span class="cp-fav__score" :class="scoreColor(facility.trust_score)">{{ facility.trust_score }}</span>
              <span class="cp-fav__jurisdiction">{{ facility.jurisdiction }}</span>
            </div>
          </div>
          <div class="cp-fav__votes">
            <button
              class="cp-vote-btn"
              :disabled="isVoting(facility.id)"
              @click="submitVote(facility.id, 'like')"
              :aria-label="`Recommend ${facility.name}`"
            >
              <ThumbsUp16 /> {{ facility.likes ?? 0 }}
            </button>
          </div>
        </li>
      </ol>
    </section>

    <!-- ─── Data sources ─── -->
    <section id="data-sources" class="cp-panel cp-sources">
      <header class="cp-section-head">
        <h2 class="cp-section-title">
          <Information16 class="cp-section-icon" /> Our Sources
        </h2>
      </header>
      <p class="cp-section-desc">
        CleanPlated aggregates official public health inspection records from county and city agencies
        across Southern California — the same data regulators use, made accessible for everyone.
      </p>

      <div class="cp-source-list">
        <div v-for="connector in connectorRows" :key="connector.source" class="cp-source">
          <div class="cp-source__header">
            <CheckmarkFilled16 v-if="!connector.error" class="cp-source__status cp-source__status--ok" />
            <WarningAltFilled16 v-else class="cp-source__status cp-source__status--warn" />
            <span class="cp-source__name">{{ formatSourceName(connector.source) }}</span>
          </div>
          <span v-if="!connector.error" class="cp-source__count">
            {{ connector.fetched_records.toLocaleString() }} records
          </span>
          <span v-else class="cp-source__error">Temporarily unavailable</span>
        </div>
      </div>

      <p class="cp-meta">Last updated: {{ lastRefreshLabel }}</p>
    </section>
  </main>

  <!-- ─── Footer ─── -->
  <footer class="cp-footer">
    <div class="cp-footer__inner">
      <p class="cp-footer__brand">CleanPlated</p>
      <p class="cp-footer__copy">
        Making public health data accessible, transparent, and useful for everyone.
      </p>
      <p class="cp-footer__credit">
        Built by
        <img src="/omega-purple.svg" alt="Cipher Labs" class="cp-footer__logo" loading="lazy" />
        <a href="https://thecipherlabs.com" target="_blank" rel="noopener noreferrer">Cipher Labs</a>
      </p>
      <nav class="cp-footer__links" aria-label="Footer">
        <a href="#data-sources">Sources</a>
        <a href="https://thecipherlabs.com" target="_blank" rel="noopener noreferrer">Cipher Labs</a>
      </nav>
      <p class="cp-footer__legal">&copy; {{ new Date().getFullYear() }} CleanPlated. All rights reserved.</p>
    </div>
  </footer>
</template>
