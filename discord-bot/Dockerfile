FROM alpine:3.19
# Why is gcc necessary? Is it for the migration?
RUN apk add --no-cache openssl gcc
COPY app/bin /var/app
RUN chmod +x /var/app/discord-bot
HEALTHCHECK NONE
ENTRYPOINT [ "/var/app/discord-bot" ]