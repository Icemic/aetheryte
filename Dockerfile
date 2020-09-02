FROM rust:1.46.0-alpine3.12

RUN apk add --update alpine-sdk

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM alpine:3.12

WORKDIR /
COPY --from=0 /app/target/release/awaki /usr/local/bin

EXPOSE 3333
CMD ["awaki"]
