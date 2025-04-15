FROM samply/secret-sync-local:main AS secret-sync
FROM alpine AS chmodder
ARG TARGETARCH
ARG FEATURE
COPY /artifacts/binaries-$TARGETARCH$FEATURE/rusthead /app/rusthead
RUN chmod +x /app/*

FROM gcr.io/distroless/cc-debian12
COPY --from=secret-sync /usr/local/bin/proxy /usr/local/bin/proxy
COPY --from=secret-sync /usr/local/bin/local /usr/local/bin/local
COPY --from=chmodder /app/rusthead /usr/local/bin/rusthead
ENV RUST_BACKTRACE=1
ENTRYPOINT [ "/usr/local/bin/rusthead" ]