# Setup the dev environment for live reloading.
# You have to manually bind to the volume for this to work.
# Keep in mind that you still have to manually test the production environment,
# since there's no separation of workspaces here.
FROM alpine:3
RUN apk add --no-cache rust cargo openssl openssl-dev pkgconfig
RUN cargo install cargo-watch
VOLUME /var/app
WORKDIR /var/app/discord-bot
ENV RUST_BACKTRACE=1
ENTRYPOINT ["/root/.cargo/bin/cargo-watch", "-x", "run"]