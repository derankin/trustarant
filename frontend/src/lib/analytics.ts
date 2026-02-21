type Primitive = string | number | boolean
type EventParams = Record<string, Primitive | null | undefined>

declare global {
  interface Window {
    dataLayer?: Array<Record<string, unknown>>
    gtag?: (...args: unknown[]) => void
  }
}

const MAX_STRING_LENGTH = 120

const sanitizeValue = (value: Primitive): Primitive => {
  if (typeof value === 'string') {
    return value.slice(0, MAX_STRING_LENGTH)
  }
  return value
}

const sanitizeParams = (params: EventParams = {}) =>
  Object.entries(params).reduce<Record<string, Primitive>>((acc, [key, value]) => {
    if (value === null || value === undefined) return acc
    acc[key] = sanitizeValue(value)
    return acc
  }, {})

export const trackEvent = (eventName: string, params: EventParams = {}) => {
  if (typeof window === 'undefined') return

  const safeParams = sanitizeParams(params)

  try {
    if (typeof window.gtag === 'function') {
      window.gtag('event', eventName, safeParams)
      return
    }

    if (Array.isArray(window.dataLayer)) {
      window.dataLayer.push({ event: eventName, ...safeParams })
    }
  } catch {
    // Analytics failures must never block UX.
  }
}

