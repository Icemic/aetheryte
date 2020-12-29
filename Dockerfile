FROM rust:1.48-slim-buster

RUN apt update && apt install -y --no-install-recommends build-essential libssl-dev pkg-config

RUN cargo install sccache

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM debian:buster-slim

WORKDIR /
COPY --from=0 /app/target/release/aetheryte /usr/local/bin

EXPOSE 3333 5353
CMD ["aetheryte"]
