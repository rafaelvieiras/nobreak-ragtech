# syntax=docker/dockerfile:1.7
#
# Imagem mínima (`FROM scratch`, só o binário estático, ~1 MB) para amd64, arm64,
# armv7 (Raspberry Pi 2/3/4 em 32 bits) e armv6 (Pi Zero/1).
# Todo o build roda na arquitetura do builder e cross-compila com rust-lld,
# então não precisa de QEMU: `docker buildx build --platform linux/arm/v7 .` é rápido.

FROM --platform=$BUILDPLATFORM node:22-alpine AS painel
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY frontend/ ./
RUN npm run build

FROM --platform=$BUILDPLATFORM rust:1.88-slim AS servico
ARG TARGETARCH
ARG TARGETVARIANT
WORKDIR /src
RUN set -eu; \
    case "${TARGETARCH}/${TARGETVARIANT}" in \
      amd64/*)  T=x86_64-unknown-linux-musl ;; \
      arm64/*)  T=aarch64-unknown-linux-musl ;; \
      arm/v7)   T=armv7-unknown-linux-musleabihf ;; \
      arm/v6)   T=arm-unknown-linux-musleabihf ;; \
      *) echo "plataforma não suportada: ${TARGETARCH}/${TARGETVARIANT}" >&2; exit 1 ;; \
    esac; \
    echo "$T" > /alvo; \
    rustup target add "$T"
COPY .cargo .cargo
COPY backend backend
COPY --from=painel /src/frontend/dist frontend/dist
RUN cd backend && cargo build --release --locked --target "$(cat /alvo)" \
 && cp "target/$(cat /alvo)/release/nobreak-ragtech" /nobreak-ragtech

FROM scratch
LABEL org.opencontainers.image.title="nobreak-ragtech" \
      org.opencontainers.image.description="Monitor open source para nobreaks Ragtech: lê a porta USB e serve um painel web" \
      org.opencontainers.image.source="https://github.com/rafaelvieiras/nobreak-ragtech" \
      org.opencontainers.image.licenses="MIT"
COPY --from=servico /nobreak-ragtech /nobreak-ragtech
USER 65532:65532
ENV NOBREAK_ESCUTA=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD ["/nobreak-ragtech", "--saude"]
ENTRYPOINT ["/nobreak-ragtech"]
