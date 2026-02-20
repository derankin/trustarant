# Trustarant Research: Southern California Food Safety Data Strategy

## Objective
Trustarant is a mobile-first directory that unifies fragmented Southern California food inspection datasets into one searchable product with a normalized 0-100 Trust Score.

## Regional Problem
Food safety data is distributed across county and city systems with different formats, update frequencies, and access patterns. A single product experience requires:
- multi-source ingestion
- schema normalization
- score harmonization
- deduplication across overlapping jurisdictions

## Jurisdictions and Access Patterns

| Jurisdiction | Primary access pattern | Typical format |
|---|---|---|
| Los Angeles County | Open data portal | CSV/JSON/API endpoints |
| San Diego County | Socrata SODA API | JSON/CSV/API queries |
| Long Beach | Open data REST API | JSON |
| Riverside County | LIVES batch files + portal exports | ZIP CSV |
| San Bernardino County | ArcGIS/EZ Inspect exports | CSV/KML/Shapefile |
| Orange County | Public portal + CPRA exports | Portal records/PDF/CSV exports |
| Pasadena | Public search portal + CPRA exports | Search interface/export |

## Recommended Backend Strategy
1. API-first ingestion for LA, San Diego, and Long Beach.
2. Batch LIVES ingestion for Riverside and San Bernardino.
3. Quarterly CPRA acquisition workflow for Orange County and Pasadena.
4. Daily cloud-scheduled normalization job producing Trust Scores.
5. Geospatial query support for proximity searches ("safest within 1 mile").

## Normalization Model
Input systems include:
- raw 100-point deductive scores
- letter grades
- placard statuses (Green/Yellow/Red)

Target output:
- single Trust Score `0-100`
- consistent facility identity
- comparable ranking across jurisdictions

Example mapping baseline:
- Numeric score: direct clamp to `0-100`
- Letter grade: A `95`, B `84`, C `74`
- Placard: Green `95`, Yellow `74`, Red `40`

## Compliance and Product Considerations
- Maintain CPRA request cadence and ingestion audit logs.
- Implement CCPA controls for user location and profile data retention.
- Capture closure and critical violation events for user notifications.
- Keep traceability from each Trust Score back to source inspection fields.

## References
- LA County Open Data Portal: https://data.lacounty.gov/
- San Diego County Open Data (Socrata): https://data.sandiegocounty.gov/
- Orange County Food Info Portal: https://ocfoodinfo.com/
- Orange County Public Records Requests (NextRequest): https://www.nextrequest.com/
- Riverside County Environmental Health: https://rivcoeh.org/
- San Bernardino County EZ Inspect: https://wp.sbcounty.gov/dph/programs/ehs/ezinspect/
- Long Beach Open Data: https://data.longbeach.gov/
- Pasadena Public Health Inspection Portal (DecadeOnline): https://www.cityofpasadena.net/public-health/food-safety-program/
- California Public Records Act Overview: https://oag.ca.gov/public-records
- LIVES Standard background (Code for America context): https://www.codeforamerica.org/
