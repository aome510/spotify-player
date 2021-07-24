FROM lukemathwalker/cargo-chef:latest-rust-1.53 as planner
WORKDIR app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM lukemathwalker/cargo-chef:latest-rust-1.53 as cacher
WORKDIR app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1.53 as builder
WORKDIR app
COPY . .
COPY --from=cacher /app/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME
RUN cargo build --release --bin spotify_player

FROM scratch
WORKDIR app
COPY --from=builder /app/target/release/spotify_player .
CMD ["./spotify_player", "-c", "/app"]
