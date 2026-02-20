<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

type FacilitySummary = {
  id: string
  name: string
  address: string
  city: string
  state: string
  postal_code: string
  jurisdiction: string
  trust_score: number
  latest_inspection_at?: string
}

const facilities = ref<FacilitySummary[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const search = ref('')
const radiusMiles = ref(2)

const laLatitude = 34.0522
const laLongitude = -118.2437
const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:8080'

const scoreBand = (score: number) => {
  if (score >= 90) return 'text-mint-500 bg-mint-500/12 ring-mint-500/40'
  if (score >= 80) return 'text-clay-600 bg-clay-500/12 ring-clay-500/40'
  return 'text-alert-500 bg-alert-500/10 ring-alert-500/30'
}

const featured = computed(() => facilities.value[0])

const formattedTime = (value?: string) => {
  if (!value) return 'No recent inspection date'
  return new Date(value).toLocaleDateString()
}

async function fetchFacilities() {
  loading.value = true
  error.value = null

  const query = new URLSearchParams({
    latitude: String(laLatitude),
    longitude: String(laLongitude),
    radius_miles: String(radiusMiles.value),
    limit: '25',
  })

  if (search.value.trim()) {
    query.set('q', search.value.trim())
  }

  try {
    const response = await fetch(`${apiBaseUrl}/api/v1/facilities?${query.toString()}`)
    if (!response.ok) {
      throw new Error(`Failed to fetch (${response.status})`)
    }

    const payload = await response.json()
    facilities.value = payload.data ?? []
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Unexpected fetch error'
  } finally {
    loading.value = false
  }
}

onMounted(fetchFacilities)
</script>

<template>
  <main class="mx-auto flex min-h-screen w-full max-w-5xl flex-col gap-6 px-4 pb-12 pt-5 text-ink-900 sm:px-6">
    <header class="relative overflow-hidden rounded-3xl border border-clay-500/20 bg-white/70 p-5 shadow-lg shadow-clay-500/10 backdrop-blur sm:p-7">
      <div class="absolute -left-10 -top-10 h-32 w-32 rounded-full bg-clay-500/20 blur-2xl" />
      <div class="absolute -right-10 -bottom-12 h-40 w-40 rounded-full bg-mint-500/15 blur-3xl" />

      <p class="text-xs font-semibold uppercase tracking-[0.2em] text-clay-600">Trustarant</p>
      <h1 class="mt-1 font-display text-4xl leading-[1.04] text-ink-950 sm:text-5xl">Find safer food, faster.</h1>
      <p class="mt-3 max-w-xl text-sm text-ink-900/80 sm:text-base">
        Mobile-first directory for Southern California health inspection data, normalized into one Trust Score.
      </p>

      <form class="mt-5 grid gap-3 sm:grid-cols-[1fr_auto]" @submit.prevent="fetchFacilities">
        <label class="relative block">
          <span class="sr-only">Search restaurants</span>
          <input
            v-model="search"
            type="text"
            placeholder="Search by name, address, or ZIP"
            class="w-full rounded-2xl border border-clay-500/25 bg-white/80 px-4 py-3 text-sm outline-none transition focus:border-clay-500 focus:ring-2 focus:ring-clay-500/20"
          />
        </label>
        <button
          type="submit"
          class="rounded-2xl bg-ink-950 px-4 py-3 text-sm font-semibold text-cream-200 transition hover:bg-ink-900"
        >
          Search
        </button>
      </form>

      <div class="mt-4 rounded-2xl bg-cream-200/65 p-3">
        <div class="flex items-center justify-between text-xs font-medium uppercase tracking-wide text-ink-900/70">
          <span>Radius</span>
          <span>{{ radiusMiles.toFixed(1) }} mi</span>
        </div>
        <input
          v-model.number="radiusMiles"
          type="range"
          min="0.5"
          max="10"
          step="0.5"
          class="mt-2 w-full accent-clay-500"
          @change="fetchFacilities"
        />
      </div>
    </header>

    <section
      v-if="featured"
      class="rounded-3xl border border-mint-500/25 bg-white p-4 shadow-md shadow-mint-500/10 sm:p-6"
    >
      <p class="text-xs font-semibold uppercase tracking-[0.16em] text-mint-500">Top in your radius</p>
      <div class="mt-2 flex items-start justify-between gap-4">
        <div>
          <h2 class="font-display text-2xl text-ink-950">{{ featured.name }}</h2>
          <p class="mt-1 text-sm text-ink-900/75">{{ featured.address }}, {{ featured.city }} {{ featured.postal_code }}</p>
          <p class="mt-2 text-xs font-medium text-ink-900/65">Last inspection: {{ formattedTime(featured.latest_inspection_at) }}</p>
        </div>
        <span :class="scoreBand(featured.trust_score)" class="rounded-xl px-3 py-2 text-lg font-bold ring-1">
          {{ featured.trust_score }}
        </span>
      </div>
    </section>

    <section class="space-y-3">
      <header class="flex items-end justify-between">
        <h3 class="font-display text-2xl text-ink-950">Directory</h3>
        <span class="text-sm text-ink-900/65">{{ facilities.length }} result(s)</span>
      </header>

      <p v-if="loading" class="rounded-2xl bg-white/70 px-4 py-3 text-sm">Loading latest trust scores...</p>
      <p v-else-if="error" class="rounded-2xl bg-alert-500/10 px-4 py-3 text-sm text-alert-500">{{ error }}</p>
      <p v-else-if="facilities.length === 0" class="rounded-2xl bg-white/70 px-4 py-3 text-sm">No facilities matched this search.</p>

      <ul v-else class="grid gap-3">
        <li
          v-for="facility in facilities"
          :key="facility.id"
          class="rounded-2xl border border-ink-900/10 bg-white/80 p-4 shadow-sm shadow-ink-900/5 transition hover:-translate-y-0.5"
        >
          <div class="flex items-start justify-between gap-4">
            <div>
              <h4 class="text-base font-semibold text-ink-950">{{ facility.name }}</h4>
              <p class="mt-1 text-sm text-ink-900/75">
                {{ facility.address }}, {{ facility.city }} {{ facility.postal_code }}
              </p>
              <p class="mt-1 text-xs uppercase tracking-wide text-ink-900/55">{{ facility.jurisdiction }}</p>
            </div>
            <span :class="scoreBand(facility.trust_score)" class="rounded-lg px-3 py-1.5 text-sm font-semibold ring-1">
              {{ facility.trust_score }}
            </span>
          </div>
        </li>
      </ul>
    </section>
  </main>
</template>
