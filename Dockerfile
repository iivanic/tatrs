# syntax=docker/dockerfile:1
FROM rust:1.92-bookworm AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --release --locked --bin tatrs

FROM debian:bookworm-slim

RUN useradd --create-home --uid 10001 tatrs
COPY --from=build /src/target/release/tatrs /usr/local/bin/tatrs

USER tatrs
WORKDIR /work
ENTRYPOINT ["tatrs"]
CMD ["--help"]
