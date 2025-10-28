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
COPY --from=git /usr/lib/git-core/git-remote-https /usr/lib/git-core/git-remote-https

COPY --from=git /usr/lib/x86_64-linux-gnu/libcurl-gnutls.so.4 \
    /usr/lib/x86_64-linux-gnu/libnghttp2.so.14 \
    /usr/lib/x86_64-linux-gnu/libidn2.so.0 \
    /usr/lib/x86_64-linux-gnu/librtmp.so.1 \
    /usr/lib/x86_64-linux-gnu/libssh2.so.1 \
    /usr/lib/x86_64-linux-gnu/libpsl.so.5 \
    /usr/lib/x86_64-linux-gnu/libnettle.so.8 \
    /usr/lib/x86_64-linux-gnu/libgnutls.so.30 \
    /usr/lib/x86_64-linux-gnu/libgssapi_krb5.so.2 \
    /usr/lib/x86_64-linux-gnu/libldap-2.5.so.0 \
    /usr/lib/x86_64-linux-gnu/liblber-2.5.so.0 \
    /usr/lib/x86_64-linux-gnu/libzstd.so.1 \
    /usr/lib/x86_64-linux-gnu/libbrotlidec.so.1 \
    /usr/lib/x86_64-linux-gnu/libunistring.so.2 \
    /usr/lib/x86_64-linux-gnu/libhogweed.so.6 \
    /usr/lib/x86_64-linux-gnu/libgmp.so.10 \
    /usr/lib/x86_64-linux-gnu/libp11-kit.so.0 \
    /usr/lib/x86_64-linux-gnu/libtasn1.so.6 \
    /usr/lib/x86_64-linux-gnu/libkrb5.so.3 \
    /usr/lib/x86_64-linux-gnu/libk5crypto.so.3 \
    /usr/lib/x86_64-linux-gnu/libcom_err.so.2 \
    /usr/lib/x86_64-linux-gnu/libkrb5support.so.0 \
    /usr/lib/x86_64-linux-gnu/libsasl2.so.2 \
    /usr/lib/x86_64-linux-gnu/libbrotlicommon.so.1 \
    /usr/lib/x86_64-linux-gnu/libffi.so.8 \
    /usr/lib/x86_64-linux-gnu/libpcre2-8.so.0 \
    /usr/lib/x86_64-linux-gnu/libz.so.1 \
    /lib/x86_64-linux-gnu/libkeyutils.so.1 \
    /usr/lib/x86_64-linux-gnu/

COPY --from=secret-sync /usr/local/bin/proxy /usr/local/bin/proxy
COPY --from=secret-sync /usr/local/bin/local /usr/local/bin/local
COPY --from=chmodder /app/rusthead /usr/local/bin/rusthead
ENV RUST_BACKTRACE=1
ENTRYPOINT [ "/usr/local/bin/rusthead" ]
