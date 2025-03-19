FROM alpine AS chmodder
ARG TARGETARCH
ARG FEATURE
COPY /artifacts/binaries-$TARGETARCH$FEATURE/rusthead /app/rusthead
RUN chmod +x /app/*

FROM gcr.io/distroless/cc-debian12
ARG COMPONENT
COPY --from=chmodder /app/rusthead /usr/local/bin/rusthead
ENTRYPOINT [ "/usr/local/bin/rusthead" ]