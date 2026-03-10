FROM rust:1.82-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY crates/star-backend crates/star-backend

# Create a minimal workspace Cargo.toml for the builder
RUN echo '[workspace]\nmembers = ["crates/star-backend"]\nresolver = "2"\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n\n[workspace.dependencies]\ntokio = { version = "1", features = ["full"] }\nserde = { version = "1", features = ["derive"] }\nserde_json = "1"\ntracing = "0.1"\ntracing-subscriber = { version = "0.3", features = ["env-filter"] }' > Cargo.toml

RUN cargo build --release --package star-backend

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/star-backend /usr/local/bin/star-backend

ENV DATABASE_URL=sqlite:/data/star.db?mode=rwc
ENV PORT=8080
EXPOSE 8080

VOLUME /data
CMD ["star-backend"]
