FROM ekidd/rust-musl-builder:latest as builder
ADD --chown=rust:rust . ./
RUN apk update && apk add libasound2
RUN cargo build --release --bin spotify_player

FROM alpine:latest
WORKDIR app
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/spotify_player .
RUN apk update && apk add libasound2
RUN mkdir -p ./config
CMD ["./spotify_player", "-c", "./config"]
