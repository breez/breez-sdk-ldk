ARG USER=lsps2

FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml ./
COPY src/ ./src

RUN cargo install --path .


FROM debian:bookworm-slim AS final

ARG USER

RUN adduser --disabled-password \
            --home "/data" \
            --gecos "" \
            "$USER"

USER $USER

COPY --from=builder /usr/local/cargo/bin/lsps2-server /usr/local/bin/

ENTRYPOINT ["lsps2-server"]
