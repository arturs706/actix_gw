FROM rust as builder

COPY . /app

WORKDIR /app

RUN cargo build --profile release-lto

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/target/release-lto/apigw /app/apigw
WORKDIR /app

CMD ["./apigw"]