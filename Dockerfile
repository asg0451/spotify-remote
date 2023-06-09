FROM debian:bullseye-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates # touch
RUN update-ca-certificates
RUN apt-get install -y libtool gstreamer1.0-tools libopus-dev ffmpeg python3 python-is-python3 gstreamer1.0-plugins-base
RUN apt-get install -y ffmpeg curl dnsutils
RUN mkdir /utils/

FROM rust:bullseye AS builder
RUN mkdir /build
WORKDIR /build

RUN apt-get update && apt-get install -y build-essential cmake
RUN apt-get install -y libtool libopus-dev

RUN rustup update

RUN mkdir -p -m 0700 /root/.ssh
RUN ssh-keyscan github.com >> /root/.ssh/known_hosts

COPY . .

RUN mkdir bin

RUN --mount=type=cache,target=/volume/target \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=ssh \
    cargo build  --config net.git-fetch-with-cli=true --release --target-dir /volume/target && \
    mv /volume/target/release/receiver bin/ && \
    mv /volume/target/release/player bin/

FROM runtime
WORKDIR /app
COPY --from=builder /build/bin/receiver /usr/local/bin/
COPY --from=builder /build/bin/player /usr/local/bin/

EXPOSE 8080

ENTRYPOINT [ "/usr/local/bin/receiver" ]
