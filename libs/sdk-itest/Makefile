.PHONY: test
test:
# Set stack size to 16MB, otherwise the test fails with stack overflow.
	RUST_MIN_STACK=16777216 cargo test -- --nocapture

.PHONY: build-images
build-images: build-vss build-rgs build-lsp

.PHONY: build-vss
build-vss:
	cd docker/vss && docker build . -t vss:latest

.PHONY: build-rgs
build-rgs:
	cd docker/rgs && docker build . -t rgs:latest

.PHONY: build-lsp
build-lsp:
	cd lsps2-server && docker build . -t lsps2-server:latest -f ../docker/lsps2-server/Dockerfile
