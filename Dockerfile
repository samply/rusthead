FROM samply/secret-sync-local:main AS secret-sync
FROM alpine AS chmodder
ARG TARGETARCH
ARG FEATURE
COPY /artifacts/binaries-$TARGETARCH$FEATURE/rusthead /app/rusthead
RUN chmod +x /app/*

FROM gcr.io/distroless/cc-debian12
ADD --chmod=+x https://github.com/docker/compose/releases/download/v2.40.0/docker-compose-linux-x86_64 /usr/local/bin/docker-compose
COPY --from=secret-sync /usr/local/bin/proxy /usr/local/bin/proxy
COPY --from=secret-sync /usr/local/bin/local /usr/local/bin/local
COPY --from=chmodder /app/rusthead /usr/local/bin/rusthead
ENV RUST_BACKTRACE=1
ENTRYPOINT [ "/usr/local/bin/rusthead" ]