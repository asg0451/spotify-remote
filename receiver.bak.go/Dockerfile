FROM golang:1.20-bullseye AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y make protobuf-compiler

COPY Makefile  ./
RUN make install-pb-tools

COPY go.mod go.sum  ./
RUN go mod download

COPY ./src ./src
RUN make build

FROM debian:bullseye-slim
RUN apt-get update
RUN apt-get install -y ca-certificates
RUN update-ca-certificates
RUN apt-get install -y curl htop dnsutils file wget # for debugging

# download dict
RUN wget https://raw.githubusercontent.com/dwyl/english-words/master/words.txt -O /usr/share/dict/words # touch

COPY --from=builder /build/bin/donnerator-server /usr/local/bin/donnerator-server

# grpc, pprof
EXPOSE 8080
EXPOSE 6060

ENTRYPOINT ["/usr/local/bin/donnerator-server"]
