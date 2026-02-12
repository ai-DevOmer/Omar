# --- Frontend Build ---
FROM node:22-slim AS frontend-builder
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npm run build

# --- Backend Build ---
FROM rust:1.80-slim AS backend-builder
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev build-essential \
    libwebkit2gtk-4.0-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev

WORKDIR /app
COPY src-tauri ./src-tauri
# We build the binary for the API server
RUN cd src-tauri && cargo build --release --bin omar-ai-api

# --- Final Production Image ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 libwebkit2gtk-4.0-37 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# Copy the built backend binary
COPY --from=backend-builder /app/src-tauri/target/release/omar-ai-api /app/omar-ai-api
# Copy the built frontend
COPY --from=frontend-builder /app/dist /app/dist

# Environment variables
ENV PORT=8080
EXPOSE 8080

# Run the API server
CMD ["./omar-ai-api"]
