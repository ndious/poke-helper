# Build de l'application Rust en release.
FROM rust:1-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data/pokedex.json ./data/pokedex.json
# Les fichiers TCG sont copiés pour l'image finale, pas pour la compilation
# (le binaire embarque pokedex.json via include_str !, mais lit les cartes
# TCG au lancement).
COPY web ./web
RUN cargo build --release --bin pokedex-web --locked

# Image d'exécution minimale : le binaire et les données cartes.
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/pokedex-web ./pokedex-web
COPY data/tcg_cards.json ./data/tcg_cards.json
COPY data/tcg_sets.json ./data/tcg_sets.json
ENV PORT=8080 HOST=0.0.0.0
EXPOSE 8080
CMD ["./pokedex-web"]
