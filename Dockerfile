FROM rust:1.82-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY crates crates

RUN cargo build --release --package star-backend

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/star-backend /usr/local/bin/star-backend

ENV DATABASE_URL=sqlite:/data/star.db?mode=rwc
ENV PORT=8080
EXPOSE 8080

VOLUME /data
CMD ["star-backend"]
