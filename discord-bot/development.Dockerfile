# Setup the dev environment for live reloading.
# You have to manually bind to the volume for this to work.
# Keep in mind that you still have to manually test the production environment,
# since there's no separation of workspaces here.

# TODO: move this off of edge as soon as there's a stable version of Alpine that has Rust 1.57.0
FROM alpine:edge
RUN apk add --no-cache 'cargo>1.57' openssl openssl-dev pkgconfig librdkafka cmake make gcc g++
RUN cargo install cargo-watch
VOLUME /var/app
WORKDIR /var/app/discord-bot
ENV RUST_BACKTRACE=1
ENTRYPOINT ["/root/.cargo/bin/cargo-watch", "-x", "run "]