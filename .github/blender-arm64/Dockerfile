FROM docker.io/library/debian:sid-20261104-slim@sha256:a9c8616b730ad5e28e92f09902a372956de501511c31fe7ebc7b406ecb2be823

COPY ca-certificates.crt /tmp/runner-ca-certificates.crt
RUN printf '%s\n' \
    'Types: deb' \
    'URIs: https://snapshot.debian.org/archive/debian/20261104T000000Z/' \
    'Suites: sid' \
    'Components: main' \
    'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
    'Check-Valid-Until: no' \
    > /tmp/blender-fixtures.sources \
    && apt-get -o Dir::Etc::sourcelist=/tmp/blender-fixtures.sources -o Dir::Etc::sourceparts=- \
       -o Acquire::https::CaInfo=/tmp/runner-ca-certificates.crt update \
    && DEBIAN_FRONTEND=noninteractive apt-get -o Dir::Etc::sourcelist=/tmp/blender-fixtures.sources \
       -o Dir::Etc::sourceparts=- -o Acquire::https::CaInfo=/tmp/runner-ca-certificates.crt \
       install -y --no-install-recommends blender=5.2.2+dfsg-1 python3-cattrs python3-numpy python3-requests \
    && test "$(dpkg-query -W -f='${Architecture}:${Version}' blender)" = 'arm64:5.2.2+dfsg-1' \
    && rm -rf /var/lib/apt/lists/* /tmp/blender-fixtures.sources /tmp/runner-ca-certificates.crt
