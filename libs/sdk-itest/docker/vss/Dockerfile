ARG USER=vss
ARG VERSION=c54b2c30589c252a608594e299a58cf72ada763e
ARG REPOSITORY=https://github.com/lightningdevkit/vss-server.git

FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update -qq && \
    apt-get install -qq -y --no-install-recommends \
        ca-certificates \
        git

ARG VERSION
ARG REPOSITORY

RUN git init && \
    git remote add origin "$REPOSITORY" && \
    git fetch --depth 1 origin "$VERSION" && \
    git checkout FETCH_HEAD

RUN cargo install --path rust/server

FROM debian:bookworm-slim AS final

ARG USER

RUN adduser --disabled-password \
            --home "/data" \
            --gecos "" \
            "$USER"

USER $USER

COPY --from=builder /usr/local/cargo/bin/server /usr/local/bin/

ENTRYPOINT ["server"]
