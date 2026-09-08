FROM scratch
COPY --chmod=0755 artifacts/rusthead /usr/local/bin/rusthead
ENTRYPOINT ["/usr/local/bin/rusthead"]
