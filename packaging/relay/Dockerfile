# Build:  docker build -f packaging/relay/Dockerfile -t dnd55-relay .
# Run:    docker run -d --restart unless-stopped -p 7979:7979 dnd55-relay
FROM rust:1.97-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p dnd_relay

FROM debian:bookworm-slim
COPY --from=build /src/target/release/dnd55-relay /usr/local/bin/dnd55-relay
USER nobody
EXPOSE 7979
ENTRYPOINT ["/usr/local/bin/dnd55-relay", "--bind", "0.0.0.0:7979"]
