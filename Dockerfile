FROM ekidd/rust-musl-builder:latest as builder

ARG BUILD_TARGET=x86_64-unknown-linux-musl

WORKDIR /home/rust/src
RUN cargo install toml-cli

COPY . ./

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN \
    cargo test \
    && \
    cargo build --release --target "${BUILD_TARGET}" \
    && \
    toml get ./Cargo.toml package.name \
    | \
    xargs -I{} mv "./target/${BUILD_TARGET}/release/{}" "app-release"

FROM ghcr.io/linuxcontainers/debian-slim:11 as prod
ARG APP=/usr/src/app

RUN apt-get update -y \
    && \
    apt-get install --no-install-recommends -y \
    ca-certificates=20210119 \
    tzdata=2021a-1+deb11u5 \
    && \
    rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && \
    useradd -g $APP_USER $APP_USER \
    && \
    mkdir -p ${APP}

COPY --from=builder /home/rust/src/app-release ${APP}/app

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./app"]
