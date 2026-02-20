FROM rust:1.92-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.toml
COPY src src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --create-home --shell /usr/sbin/nologin app

WORKDIR /app
COPY --from=builder /app/target/release/trustarant-backend /usr/local/bin/trustarant-backend

ENV TRUSTARANT_HOST=0.0.0.0
ENV TRUSTARANT_PORT=8080

EXPOSE 8080
USER app

CMD ["trustarant-backend"]
