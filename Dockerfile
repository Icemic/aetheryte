FROM rust:1.48-slim-buster

RUN apt-get update && apt-get install -y build-essential

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM debian:buster-slim

RUN apt-get update && apt-get install -y wget curl

WORKDIR /
COPY --from=0 /app/target/release/aetheryte /usr/local/bin

EXPOSE 3333 5353
CMD ["aetheryte"]
