# builder
FROM docker.io/rust:alpine as builder
WORKDIR /workspace
RUN apk add --no-cache musl-dev sqlite

# build deps
COPY Cargo.toml Cargo.toml
RUN mkdir src/
RUN echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs
RUN cargo build --release
RUN rm -f /workspace/target/release/deps/just_recive_and_post*

# build apps
COPY . .
RUN cargo build --release

# runner
FROM docker.io/alpine:latest
RUN addgroup -g 1000 just-recive-and-post
RUN adduser -D -s /bin/sh -u 1000 -G just-recive-and-post just-recive-and-post

WORKDIR /app
COPY --from=builder /workspace/target/release/just-recive-and-post /app/just-recive-and-post
COPY --from=builder /workspace/static /app/static
COPY --from=builder /workspace/db /app/db
COPY --from=builder /workspace/Rocket.toml /app/Rocket.toml

RUN chown just-recive-and-post:just-recive-and-post just-recive-and-post
USER just-recive-and-post
CMD ["/app/just-recive-and-post"]
