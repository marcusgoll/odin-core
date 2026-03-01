FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build -p odin-cli --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 10001 odin
RUN mkdir -p /var/odin && chown odin:odin /var/odin
WORKDIR /app
COPY --from=build /app/target/release/odin-cli /usr/local/bin/odin-cli
USER odin
ENTRYPOINT ["/usr/local/bin/odin-cli"]
