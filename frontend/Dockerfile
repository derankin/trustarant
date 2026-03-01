FROM node:24-alpine AS build
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .
ARG VITE_API_BASE_URL=http://localhost:8080
ARG VITE_GOOGLE_MAPS_API_KEY=
ENV VITE_API_BASE_URL=${VITE_API_BASE_URL}
ENV VITE_GOOGLE_MAPS_API_KEY=${VITE_GOOGLE_MAPS_API_KEY}
RUN npm run build

FROM node:24-alpine
WORKDIR /app
ENV NODE_ENV=production

COPY package.json package-lock.json ./
RUN npm ci --omit=dev

COPY server.mjs ./server.mjs
COPY --from=build /app/dist ./dist

ARG VITE_API_BASE_URL=http://localhost:8080
ENV API_BASE_URL=${VITE_API_BASE_URL}
ENV PUBLIC_BASE_URL=https://cleanplated.com

EXPOSE 8080
CMD ["node", "server.mjs"]
