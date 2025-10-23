.PHONY: build-images
build-images: build-vss build-lsp

.PHONY: build-vss
build-vss:
	cd docker/vss && docker build . -t vss:latest

.PHONY: build-lsp
build-lsp:
	cd lsps2-server && docker build . -t lsps2-server:latest -f ../docker/lsps2-server/Dockerfile
