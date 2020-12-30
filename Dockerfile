FROM rust:1.48-slim-buster

RUN apt update && apt install -y --no-install-recommends build-essential libssl-dev pkg-config

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM ubuntu:bionic

WORKDIR /
COPY --from=0 /app/target/release/aetheryte /usr/local/bin

EXPOSE 3333 5353
CMD ["aetheryte"]
