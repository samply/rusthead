FROM samply/secret-sync-local:main AS secret-sync
FROM alpine AS chmodder
ARG TARGETARCH
ARG FEATURE
COPY /artifacts/binaries-$TARGETARCH$FEATURE/rusthead /app/rusthead
RUN chmod +x /app/*

FROM debian:bookworm-slim AS git

RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*

FROM gcr.io/distroless/cc-debian12
ADD --chmod=+x https://github.com/docker/compose/releases/download/v2.40.0/docker-compose-linux-x86_64 /usr/local/bin/docker-compose

COPY --from=git /usr/bin/git /usr/bin/git
COPY --from=git /usr/lib/x86_64-linux-gnu/libpcre2-8.so.0 /usr/lib/x86_64-linux-gnu/
COPY --from=git /usr/lib/x86_64-linux-gnu/libz.so.1 /usr/lib/x86_64-linux-gnu/

COPY --from=secret-sync /usr/local/bin/proxy /usr/local/bin/proxy
COPY --from=secret-sync /usr/local/bin/local /usr/local/bin/local
COPY --from=chmodder /app/rusthead /usr/local/bin/rusthead
ENV RUST_BACKTRACE=1
ENTRYPOINT [ "/usr/local/bin/rusthead" ]