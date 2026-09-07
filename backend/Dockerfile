FROM rust:1.96.1-trixie AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release

FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/coffee-tracker /usr/local/bin/coffee-tracker
COPY --from=builder /app/target/release/coffee-admin /usr/local/bin/coffee-admin
ENV DATABASE_URL=sqlite:///data/coffee-tracker.db
ENV BIND_ADDR=0.0.0.0:3124
ENV TZ=Europe/Berlin
VOLUME ["/data"]
EXPOSE 3124
CMD ["coffee-tracker"]
