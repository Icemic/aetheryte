FROM rust:1.48-slim-buster

RUN apt-get update && apt-get install -y --no-install-recommends build-essential

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM ubuntu:bionic

RUN apt-get update && apt-get install -y --no-install-recommends wget curl

WORKDIR /
COPY --from=0 /app/target/release/aetheryte /usr/local/bin

EXPOSE 3333 5353
CMD ["aetheryte"]
