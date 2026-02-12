# Build stage for Frontend
FROM node:22-slim AS frontend-builder
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npm run build

# Build stage for Rust Backend (Tauri)
FROM rust:1.80-slim AS backend-builder
RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    pkg-config

WORKDIR /app
COPY src-tauri ./src-tauri
# Copy built frontend from previous stage
COPY --from=frontend-builder /app/dist ./dist

# Note: For Railway, we typically build the web version or a server-side component 
# as Tauri is a desktop framework. If this is intended to be a web-accessible API 
# or a hosted version, we need to ensure it's configured for server execution.
# For now, we'll focus on ensuring the build process completes.

CMD ["npm", "start"]
