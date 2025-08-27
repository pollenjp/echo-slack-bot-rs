# https://hub.docker.com/_/rust#supported-tags-and-respective-dockerfile-links
FROM mirror.gcr.io/rust:1.89-bookworm AS base

WORKDIR /code
RUN cargo init
COPY Cargo.toml /code/Cargo.toml
RUN cargo fetch
COPY . /code

FROM base AS builder
RUN cargo build --release --offline

# https://hub.docker.com/_/debian#supported-tags-and-respective-dockerfile-links
FROM mirror.gcr.io/debian:12.1-slim AS release

# RUN apt-get update -y \
#     && \
#     apt-get install --no-install-recommends -y \
#     ca-certificates=20210119 \
#     tzdata=2021a-1+deb11u5 \

ENV TZ=Etc/UTC \
    APP_USER=appuser \
    BIN_NAME=echo-slack-bot-rs
COPY --from=builder /code/target/release/"${BIN_NAME}" /app

# Install ca-certificates
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd "$APP_USER" \
    && useradd -g "$APP_USER" "$APP_USER"
RUN chown "$APP_USER":"$APP_USER" /app

USER "$APP_USER"
CMD [ "/app" ]
