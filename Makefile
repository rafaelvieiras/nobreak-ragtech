.PHONY: dev-backend dev-frontend frontend backend test docker arm
frontend:
	cd frontend && npm ci && npm run build
backend: frontend
	cd backend && cargo build --release
test:
	cd backend && cargo test && cargo clippy --all-targets -- -D warnings
	cd frontend && npm run check
# Painel + serviço simulado, sem nobreak, em http://127.0.0.1:8080
dev-backend:
	cd backend && NOBREAK_SIMULAR=1 NOBREAK_ESCUTA=127.0.0.1:8080 cargo run
dev-frontend:
	cd frontend && npm run dev
docker:
	docker build -t nobreak-ragtech:dev .
# Binário estático para Raspberry Pi 32 bits (sem Docker)
arm: frontend
	rustup target add armv7-unknown-linux-musleabihf
	cd backend && cargo build --release --target armv7-unknown-linux-musleabihf
