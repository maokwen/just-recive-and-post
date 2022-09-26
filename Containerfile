# builder
FROM docker.io/rust:slim-bullseye as builder
WORKDIR /workspace
COPY ./ .
RUN cargo install --path .

FROM docker.io/debian:bullseye-slim
WORKDIR /app
COPY --from=builder /workspace/target/release/just-recive-and-post /app/just-recive-and-post
COPY --from=builder /workspace/static /app/static
COPY --from=builder /workspace/db /app/db
COPY --from=builder /workspace/Rocket.toml /app/Rocket.toml
CMD ["/app/just-recive-and-post"]

FROM docker.io/rust:slim-bullseye as builder
WORKDIR /workspace
COPY ./ .
RUN cargo install --path .

ENTRYPOINT [ "just-recive-and-post" ]
