# Checkout files and prepare everything
FROM alpine:3.16 as prepare
RUN apk add --no-cache 'cargo>1.57' openssl openssl-dev pkgconfig librdkafka cmake make gcc g++
COPY ./ /var/src

FROM prepare AS install
WORKDIR /var/src/discord-bot
RUN cargo install --root /var/app --bin discord-bot --path .

FROM alpine:3.16
# Why is gcc necessary? Is it for the migration?
RUN apk add --no-cache openssl gcc
COPY --from=install /var/app/bin /var/app
RUN chmod +x /var/app/discord-bot
ENTRYPOINT [ "/var/app/discord-bot" ]