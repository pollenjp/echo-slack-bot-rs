FROM rust:1.63 as builder

# build a template project for creating cache images
RUN USER=root cargo new --bin /app
WORKDIR /app
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release && \
    rm src/*.rs && \
    cargo install toml-cli

# copy a project and build it
COPY . /app

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN toml get ./Cargo.toml package.name | \
    sed 's/-/_/g' | \
    xargs -I{} rm -rf ./target/release/deps/{}*

RUN cargo test && \
    cargo build --release && \
    toml get ./Cargo.toml package.name | \
    xargs -I{} cp "./target/release/{}" "/app/app-release"

FROM ghcr.io/linuxcontainers/debian-slim:11 as prod
ARG APP=/usr/src/app

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=builder /app/app-release ${APP}/app

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./app"]
