FROM ekidd/rust-musl-builder:latest as builder
RUN sudo apt-get update && sudo apt-get install -y libasound2-dev
ADD --chown=rust:rust . ./
RUN cargo build --release --bin spotify_player

FROM alpine:latest
WORKDIR app
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/spotify_player .
RUN mkdir -p ./config
CMD ["./spotify_player", "-c", "./config"]
