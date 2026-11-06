ARG BUILDER_IMAGE
FROM ${BUILDER_IMAGE} AS toolchain
RUN apk add --no-cache bash alpine-sdk util-linux strace file autoconf automake libtool xz \
    eudev-dev gettext-dev linux-headers meson zstd-dev zstd-static zlib-dev zlib-static \
    clang musl-dev mimalloc2-dev python3 git
COPY packages/ /src/packages/
RUN apk add --no-cache /src/packages/*.apk
COPY upstream/ /src/runtime/
COPY Dockerfile collect-sources.py RELINK.md /src/
COPY inputs.json /src/inputs.json
ARG RUNTIME_COMMIT
RUN printf '%s\n' "$RUNTIME_COMMIT" > /src/runtime/src/runtime/version
RUN bash /src/runtime/scripts/common/install-dependencies.sh
FROM toolchain AS runtime
WORKDIR /src/runtime
RUN printf '\nCFLAGS += -Wl,-Map=/src/runtime/runtime-link.map\n' >> src/runtime/Makefile
RUN bash scripts/build-runtime.sh
FROM runtime AS distribution
RUN python3 /src/collect-sources.py --output /export --upstream /src/runtime --inputs /src/inputs.json
RUN cp -a /src/packages /export/sources/packages
RUN cp /src/Dockerfile /src/collect-sources.py /src/RELINK.md /export/sources/
ENTRYPOINT ["cp", "-a", "/export/.", "/output/"]
