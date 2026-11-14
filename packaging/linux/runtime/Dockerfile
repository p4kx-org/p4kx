ARG BUILDER_IMAGE
FROM ${BUILDER_IMAGE} AS toolchain
RUN apk add --no-cache bash alpine-sdk util-linux strace file autoconf automake libtool xz \
    eudev-dev gettext-dev linux-headers meson zstd-dev zstd-static zlib-dev zlib-static \
    clang musl-dev mimalloc2-dev python3 git
COPY packages/ /src/packages/
RUN apk add --no-cache /src/packages/*.apk
COPY upstream/ /src/runtime/
COPY Dockerfile collect-sources.py build_inputs.py RELINK.md /src/
COPY inputs.json /src/inputs.json
ARG RUNTIME_COMMIT
RUN printf '%s\n' "$RUNTIME_COMMIT" > /src/runtime/src/runtime/version
RUN python3 /src/build_inputs.py prepare --upstream /src/runtime --inputs /src/inputs.json --root /src/dependencies
RUN bash /src/runtime/scripts/common/install-dependencies.sh
RUN python3 /src/build_inputs.py record --upstream /src/runtime --inputs /src/inputs.json --root /src/dependencies
FROM toolchain AS runtime
WORKDIR /src/runtime
RUN cp src/runtime/Makefile /src/dependencies/upstream-runtime-Makefile && \
    printf '\nCFLAGS += -Wl,-Map=/src/runtime/runtime-link.map\n' >> src/runtime/Makefile
RUN bash scripts/build-runtime.sh
FROM runtime AS distribution
RUN python3 /src/collect-sources.py --output /export --upstream /src/runtime --inputs /src/inputs.json --dependencies /src/dependencies
RUN cp -a /src/packages /export/sources/packages
RUN cp /src/Dockerfile /src/collect-sources.py /src/build_inputs.py /src/RELINK.md /export/sources/
ENTRYPOINT ["cp", "-a", "/export/.", "/output/"]
