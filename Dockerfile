FROM rust:1.66 AS build

COPY . /app/
WORKDIR /app/

RUN cargo build --release

FROM debian:buster-slim AS redis-cleaner
RUN apt-get update && apt-get install -y openssl && apt-get clean
COPY --from=build /app/target/release/redis-cleaner /app/redis-cleaner
WORKDIR /app
ENTRYPOINT ["/app/redis-cleaner"]
