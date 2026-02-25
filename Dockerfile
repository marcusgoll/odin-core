FROM rust:1-bookworm as build
WORKDIR /app
COPY . .
RUN cargo build -p odin-cli --release

FROM debian:bookworm-slim
RUN useradd -m -u 10001 odin
WORKDIR /app
COPY --from=build /app/target/release/odin-cli /usr/local/bin/odin-cli
USER odin
ENTRYPOINT ["/usr/local/bin/odin-cli"]
